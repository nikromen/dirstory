//! Backend tests use an independent model and injectable storage operations.

use super::*;
use proptest::prelude::*;
use std::fs;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Outcome injected by the test storage, never by history logic.
#[derive(Clone, Copy)]
enum Action {
    Error,
    Exit,
}

/// Failure before, partway through, or after a selected mutation.
#[derive(Clone, Copy, Debug)]
enum Timing {
    Before,
    Partial,
    After,
}

/// Real disk storage with a deterministic failure on its nth mutation.
/// Reads and length queries remain unchanged, so reopening validates real bytes.
struct FaultStorage {
    inner: FileStorage,
    remaining: usize,
    timing: Timing,
    action: Action,
}

impl FaultStorage {
    fn fail(&self) -> io::Result<()> {
        match self.action {
            Action::Error => Err(io::Error::other("Injected write failure")),
            Action::Exit => std::process::exit(77),
        }
    }

    fn next(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.remaining == 0
    }
}

impl Storage for FaultStorage {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_at(offset, buf)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        if !self.next() {
            return self.inner.write_at(offset, buf);
        }
        match self.timing {
            Timing::Before => {}
            Timing::Partial => self.inner.write_at(offset, &buf[..buf.len() / 2])?,
            Timing::After => self.inner.write_at(offset, buf)?,
        }
        self.fail()
    }

    fn length(&self) -> io::Result<u64> {
        self.inner.length()
    }

    fn set_len(&mut self, len: u64) -> io::Result<()> {
        if !self.next() {
            return self.inner.set_len(len);
        }
        if matches!(self.timing, Timing::After | Timing::Partial) {
            self.inner.set_len(len)?;
        }
        self.fail()
    }
}

/// Move an initialized file into a test-only storage adapter.
fn inject(h: History, mutation: usize, timing: Timing, action: Action) -> History<FaultStorage> {
    History {
        file: FaultStorage {
            inner: h.file,
            remaining: mutation,
            timing,
            action,
        },
    }
}

/// Visit/reset perform nine mutations; commit changes only the header (five).
fn mutations(operation: &str) -> usize {
    if operation.ends_with("commit") {
        5
    } else {
        9
    }
}

/// A failure before the dirty marker or after its final clearing leaves valid data.
fn remains_valid(mutation: usize, total: usize, timing: Timing) -> bool {
    (mutation == 1 && matches!(timing, Timing::Before | Timing::Partial))
        || (mutation == total && matches!(timing, Timing::After))
}

/// Straightforward visit list, intentionally unaware of the on-disk format.
struct Model {
    paths: Vec<PathBuf>,
    cursor: usize,
}

impl Model {
    fn new(path: PathBuf) -> Self {
        Self {
            paths: vec![path],
            cursor: 0,
        }
    }

    fn visit(&mut self, old: PathBuf, new: PathBuf) {
        if self.paths[self.cursor] != old {
            *self = Self::new(old.clone());
        }
        if old != new {
            self.paths.truncate(self.cursor + 1);
            self.paths.push(new);
            self.cursor += 1;
        }
    }

    fn target(&self, back: bool, n: usize) -> Option<usize> {
        let index = if back {
            self.cursor.saturating_sub(n)
        } else {
            self.cursor.saturating_add(n).min(self.paths.len() - 1)
        };
        (n == 0 || index != self.cursor).then_some(index)
    }

    fn list(&self, back: bool, n: usize) -> Vec<PathBuf> {
        if back {
            self.paths[..self.cursor]
                .iter()
                .rev()
                .take(n)
                .cloned()
                .collect()
        } else {
            self.paths[self.cursor + 1..]
                .iter()
                .take(n)
                .cloned()
                .collect()
        }
    }
}

/// Operations generated with small repeated names as well as longer paths.
#[derive(Clone, Debug)]
enum Op {
    Visit { name: String, mismatch: bool },
    Select { back: bool, n: usize, commit: bool },
    Reset(String),
    Ensure(String),
    List(bool, usize),
    Stale,
}

fn operations() -> impl Strategy<Value = Vec<Op>> {
    let path = prop_oneof![Just("a".to_owned()), Just("b".to_owned()), "[a-z]{1,64}"];
    let op = prop_oneof![
        (path.clone(), any::<bool>()).prop_map(|(name, mismatch)| Op::Visit { name, mismatch }),
        (
            any::<bool>(),
            prop_oneof![Just(0), Just(usize::MAX), 1usize..140],
            any::<bool>()
        )
            .prop_map(|(back, n, commit)| Op::Select { back, n, commit }),
        path.clone().prop_map(Op::Reset),
        path.prop_map(Op::Ensure),
        (any::<bool>(), 0usize..140).prop_map(|(b, n)| Op::List(b, n)),
        Just(Op::Stale),
    ];
    prop::collection::vec(op, 1..=128)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct(
            "tests/backend/regressions.txt",
        ))),
        .. ProptestConfig::default()
    })]

    #[test]
    fn history_matches_reference_model(ops in operations()) {
        let dir = tempfile::tempdir().unwrap();
        let mut history = History::open(dir.path(), true).unwrap();
        let mut model = Model::new(PathBuf::from("/initial"));
        history.ensure(&model.paths[0]).unwrap();

        for (step, op) in ops.iter().enumerate() {
            match op {
                Op::Visit { name, mismatch } => {
                    let old = if *mismatch { PathBuf::from("/outside") }
                        else { model.paths[model.cursor].clone() };
                    let new = PathBuf::from(format!("/{name}"));
                    history.visit(&old,&new).unwrap(); model.visit(old,new);
                }
                Op::Select {back,n,commit} => {
                    let before = fs::read(dir.path().join("history.bin")).unwrap();
                    let selected = history.select(*back,*n).unwrap();
                    let target = model.target(*back,*n);
                    prop_assert_eq!(selected.as_ref().map(|s|s.entry.path.clone()), target.map(|i|model.paths[i].clone()));
                    prop_assert_eq!(fs::read(dir.path().join("history.bin")).unwrap(),before);
                    if *commit {
                        if let Some(s) = selected { history.commit(s.revision,s.entry.offset).unwrap(); model.cursor=target.unwrap(); }
                    }
                }
                Op::Reset(name) => { let path=PathBuf::from(format!("/{name}")); history.reset(&path).unwrap();model=Model::new(path); }
                Op::Ensure(name) => { history.ensure(Path::new(&format!("/{name}"))).unwrap(); }
                Op::List(back,n) => {
                    prop_assert_eq!(history.list(*back,*n).unwrap().into_iter().map(|e|e.path).collect::<Vec<_>>(),model.list(*back,*n));
                }
                Op::Stale => {
                    let s=history.select(true,0).unwrap().unwrap();
                    let old=model.paths[model.cursor].clone(); let new=PathBuf::from("/changed-revision");
                    history.reset(&new).unwrap(); model=Model::new(new);
                    prop_assert!(history.commit(s.revision,s.entry.offset).is_err(),"stale selection from {old:?}");
                }
            }

            if step % 8 == 0 { drop(history); history=History::open(dir.path(),true).unwrap(); }
            prop_assert_eq!(history.select(true,0).unwrap().unwrap().entry.path,model.paths[model.cursor].clone());
            for back in [true,false] {
                prop_assert_eq!(history.list(back,usize::MAX).unwrap().into_iter().map(|e|e.path).collect::<Vec<_>>(),model.list(back,usize::MAX));
            }
        }
    }
}

/// Exercise validation independently of the encoder, including both record directions.
#[test]
fn malformed_files_are_rejected_and_resettable() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    h.reset(Path::new("/abc"))?;
    let original = fs::read(dir.path().join("history.bin"))?;
    let mut cases = Vec::new();
    for len in [1, 7, 16, 63, 64, original.len() - 1] {
        cases.push((format!("truncated-{len}"), original[..len].to_vec()));
    }
    for (name, offset, value) in [
        ("magic", 0, 0),
        ("version", 8, 99),
        ("dirty", 16, 1),
        ("cursor-in-header", 24, 8),
        ("cursor-past-end", 24, u64::MAX),
        ("wrong-end", 32, 999),
        ("reserved", 48, 1),
        ("huge-length", 64, u64::MAX),
        ("trailer", original.len() - 8, 5),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        cases.push((name.to_owned(), bytes));
    }
    let mut nul = original.clone();
    nul[72] = 0;
    cases.push(("nul-path".to_owned(), nul));
    for (name, bytes) in cases {
        fs::write(dir.path().join("history.bin"), bytes)?;
        assert!(h.select(true, 0).is_err(), "{name}");
        h.reset(Path::new("/recovered"))?;
        assert_eq!(
            h.select(true, 0)?.unwrap().entry.path,
            Path::new("/recovered"),
            "{name}"
        );
    }
    Ok(())
}

/// Errors before, during and after mutations propagate and permit explicit reset.
#[test]
fn injected_write_failures_require_reset() -> io::Result<()> {
    for operation in ["visit", "reset", "commit"] {
        for timing in [Timing::Before, Timing::Partial, Timing::After] {
            for mutation in 1..=mutations(operation) {
                let dir = tempfile::tempdir()?;
                let mut h = History::open(dir.path(), true)?;
                h.reset(Path::new("/a"))?;
                h.visit(Path::new("/a"), Path::new("/b"))?;
                let s = h.select(true, 1)?.unwrap();
                let mut h = inject(h, mutation, timing, Action::Error);
                let result = match operation {
                    "visit" => h.visit(Path::new("/b"), Path::new("/c")),
                    "reset" => h.reset(Path::new("/reset")),
                    _ => h.commit(s.revision, s.entry.offset),
                };
                assert!(result.is_err(), "{operation}/{mutation}/{timing:?}");
                drop(h);
                let mut h = History::open(dir.path(), true)?;
                assert_eq!(
                    h.ensure(Path::new("/a")).is_ok(),
                    remains_valid(mutation, mutations(operation), timing),
                    "{operation}/{mutation}/{timing:?}"
                );
                if remains_valid(mutation, mutations(operation), timing) {
                    let expected = if mutation == 1 {
                        "/b"
                    } else {
                        match operation {
                            "visit" => "/c",
                            "reset" => "/reset",
                            _ => "/a",
                        }
                    };
                    assert_eq!(h.select(true, 0)?.unwrap().entry.path, Path::new(expected));
                }
                h.reset(Path::new("/recovered"))?;
                assert_eq!(
                    h.select(true, 0)?.unwrap().entry.path,
                    Path::new("/recovered")
                );
            }
        }
    }
    Ok(())
}

/// Kill and reap a child even when an assertion unwinds.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl ChildGuard {
    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "child timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn child(dir: &Path, action: &str, point: &str) -> ChildGuard {
    ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "history::backend_tests::process_helper",
                "--nocapture",
            ])
            .env("DIRSTORY_TEST_DIR", dir)
            .env("DIRSTORY_TEST_ACTION", action)
            .env("DIRSTORY_TEST_POINT", point)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Test-executable-only child entry point; no environment hooks exist in the CLI.
#[test]
fn process_helper() -> io::Result<()> {
    let Some(dir) = std::env::var_os("DIRSTORY_TEST_DIR") else {
        return Ok(());
    };
    let dir = PathBuf::from(dir);
    let action = std::env::var("DIRSTORY_TEST_ACTION").unwrap();
    if action == "lock" {
        // Establish real contention before entering the normal blocking open.
        // The parent retains its lock until it receives this acknowledgement.
        let probe = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join("history.lock"))?;
        match probe.try_lock() {
            Err(std::fs::TryLockError::WouldBlock) => fs::write(dir.join("ready"), b"ready")?,
            other => return Err(io::Error::other(format!("Expected contention: {other:?}"))),
        }
        let _history = History::open(&dir, true)?;
        fs::write(dir.join("entered"), b"entered")?;
        return Ok(());
    }
    let mut h = History::open(&dir, action != "reader")?;
    if action == "reader" {
        h.select(true, 0)?;
        return Ok(());
    }
    if action == "holder" {
        fs::write(dir.join("holder-ready"), b"ready")?;
        loop {
            std::thread::park();
        }
    }
    let selected = h.select(true, 1)?.unwrap();
    let mutation = std::env::var("DIRSTORY_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let mut h = inject(h, mutation, Timing::After, Action::Exit);
    match action.as_str() {
        "exit-reset" => h.reset(Path::new("/reset"))?,
        "exit-commit" => h.commit(selected.revision, selected.entry.offset)?,
        _ => h.visit(Path::new("/b"), Path::new("/c"))?,
    }
    panic!("storage mutation was not reached");
}

#[test]
fn abrupt_exit_at_write_boundaries_is_detected() -> io::Result<()> {
    for operation in ["exit-visit", "exit-reset", "exit-commit"] {
        for mutation in 1..=mutations(operation) {
            let dir = tempfile::tempdir()?;
            let mut h = History::open(dir.path(), true)?;
            h.reset(Path::new("/a"))?;
            h.visit(Path::new("/a"), Path::new("/b"))?;
            drop(h);
            assert_eq!(
                child(dir.path(), operation, &mutation.to_string())
                    .wait()
                    .code(),
                Some(77),
                "{operation}/{mutation}"
            );
            let mut h = History::open(dir.path(), true)?;
            assert_eq!(
                h.select(true, 0).is_ok(),
                remains_valid(mutation, mutations(operation), Timing::After),
                "{operation}/{mutation}"
            );
            h.reset(Path::new("/recovered"))?;
        }
    }
    Ok(())
}

/// Readiness proves that the child reached contention, rather than merely being slow.
#[test]
fn concurrency_writer_reaches_lock_before_release() -> io::Result<()> {
    for exclusive in [true, false] {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        h.reset(Path::new("/a"))?;
        drop(h);
        let h = History::open(dir.path(), exclusive)?;
        let mut process = child(dir.path(), "lock", "");
        wait_ready(dir.path().join("ready"), &mut process);
        assert!(!dir.path().join("entered").exists());
        drop(h);
        assert!(process.wait().success());
        assert!(dir.path().join("entered").exists());
    }
    Ok(())
}

/// Wait for explicit child readiness with a deadline and an early-exit check.
fn wait_ready(path: PathBuf, process: &mut ChildGuard) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            process.0.try_wait().unwrap().is_none(),
            "child exited before readiness"
        );
        assert!(Instant::now() < deadline, "readiness timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn concurrency_shared_readers_can_overlap() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    h.reset(Path::new("/a"))?;
    drop(h);
    let _reader = History::open(dir.path(), false)?;
    assert!(child(dir.path(), "reader", "").wait().success());
    Ok(())
}

#[test]
fn concurrency_killed_holder_releases_lock() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    h.reset(Path::new("/a"))?;
    drop(h);
    let mut holder = child(dir.path(), "holder", "");
    wait_ready(dir.path().join("holder-ready"), &mut holder);
    let mut waiter = child(dir.path(), "lock", "");
    wait_ready(dir.path().join("ready"), &mut waiter);
    assert!(!dir.path().join("entered").exists());
    holder.0.kill()?;
    holder.wait();
    assert!(waiter.wait().success());
    assert!(dir.path().join("entered").exists());
    Ok(())
}
