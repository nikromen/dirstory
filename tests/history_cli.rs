use std::{
    fs,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

/// Isolated CLI environment with a dedicated runtime directory and tmux identifier.
struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("config")).unwrap();
        fs::write(
            root.path().join("config/dirstory.json"),
            r#"{"mode":"tmux","unique_top":true}"#,
        )
        .unwrap();
        Self { root }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_dirstory"));
        c.args(args)
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("XDG_RUNTIME_DIR", self.root.path())
            .env("TMUX_PANE", "%integration");
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        let o = self.output(args);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        o
    }

    /// Bound every CLI subprocess and reap it when a test unwinds.
    fn output(&self, args: &[&str]) -> Output {
        bounded_output(self.command(args))
    }

    fn data(&self) -> PathBuf {
        self.root.path().join("dirstory/integration/history.bin")
    }
}

#[test]
fn selection_is_read_only_and_commit_changes_only_header() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    f.run(&["internal", "visit", "--from", "/a", "--to", "/b"]);
    let before = fs::read(f.data()).unwrap();
    let s = f.run(&["internal", "select", "back", "1"]);
    assert_eq!(before, fs::read(f.data()).unwrap());
    let text = String::from_utf8(s.stdout).unwrap();
    let (token, path) = text.split_once('\n').unwrap();
    assert_eq!(path, "/a\n");
    f.run(&["internal", "commit", token]);
    let after = fs::read(f.data()).unwrap();
    assert_eq!(before[64..], after[64..]);
    assert_ne!(before[..64], after[..64]);
    assert!(!f.output(&["internal", "commit", token]).status.success());
}

#[test]
fn interrupted_write_is_rejected_and_reset_recovers() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    let mut file = OpenOptions::new().write(true).open(f.data()).unwrap();
    file.seek(SeekFrom::Start(16)).unwrap();
    file.write_all(&1u64.to_le_bytes()).unwrap();
    let result = f.output(&["internal", "select", "back", "1"]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("reset"));
    f.run(&["internal", "reset", "/b"]);
    assert_eq!(f.run(&["internal", "list", "back", "10"]).stdout, b"");
}

#[test]
fn lf_is_rejected_before_mutation_and_sessions_are_isolated() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    let before = fs::read(f.data()).unwrap();
    assert!(!f
        .output(&["internal", "visit", "--from", "/a", "--to", "/b\nc"])
        .status
        .success());
    assert_eq!(before, fs::read(f.data()).unwrap());
    let mut command = f.command(&["internal", "ensure", "/other"]);
    command.env("TMUX_PANE", "%other");
    let result = bounded_output(command);
    assert!(result.status.success());
    assert_eq!(before, fs::read(f.data()).unwrap());
}

#[test]
fn internal_protocol_is_hidden_but_has_debug_help() {
    let f = Fixture::new();
    let help = f.run(&["--help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("init"));
    assert!(!help.contains("internal"));
    let help = f.run(&["internal", "--help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    for command in ["ensure", "visit", "select", "commit", "list", "reset"] {
        assert!(help.contains(command));
    }
    for old in ["history", "navigate"] {
        assert!(!f.output(&[old, "--help"]).status.success());
    }
}

/// Ensure a CLI child is terminated and reaped if its deadline or assertion fails.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn bounded_output(mut command: Command) -> Output {
    let mut child = ChildGuard(Some(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    // Drain both pipes concurrently, so even unexpectedly verbose errors cannot block exit.
    let stdout = child.0.as_mut().unwrap().stdout.take().unwrap();
    let stderr = child.0.as_mut().unwrap().stderr.take().unwrap();
    let read = |mut pipe: Box<dyn std::io::Read + Send>| {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).unwrap();
        bytes
    };
    let out = std::thread::spawn(move || read(Box::new(stdout)));
    let err = std::thread::spawn(move || read(Box::new(stderr)));
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "CLI subprocess timed out: {command:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    child.0.take();
    Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// Invalid requests must not mutate state or leak protocol data to stdout.
#[test]
fn invalid_cli_arguments_leave_history_unchanged() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    f.run(&["internal", "visit", "--from", "/a", "--to", "/b"]);
    let before = fs::read(f.data()).unwrap();
    let cases: &[&[&str]] = &[
        &["internal", "select", "back", "-1"],
        &["internal", "select", "back", "nope"],
        &["internal", "select", "sideways", "1"],
        &["internal", "list", "forward", "-1"],
        &["internal", "list", "back", "184467440737095516160"],
        &["internal", "commit", ""],
        &["internal", "commit", "1"],
        &["internal", "commit", "1:2:3"],
        &["internal", "commit", "-1:64"],
        &["internal", "commit", "999:64"],
        &["internal", "reset", "/bad\npath"],
        &["internal", "ensure", "/bad\npath"],
        &["internal", "visit", "--from", "/bad\npath", "--to", "/b"],
    ];
    for args in cases {
        let result = f.output(args);
        assert!(!result.status.success(), "{args:?}");
        assert!(result.stdout.is_empty(), "{args:?}");
        assert!(!result.stderr.is_empty(), "{args:?}");
        assert_eq!(before, fs::read(f.data()).unwrap(), "{args:?}");
    }
}

#[test]
fn ensure_preserves_state_and_reset_invalidates_tokens() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    f.run(&["internal", "visit", "--from", "/a", "--to", "/b"]);
    let selection = f.run(&["internal", "select", "back", "1"]);
    let token = std::str::from_utf8(&selection.stdout)
        .unwrap()
        .split_once('\n')
        .unwrap()
        .0;
    let before = fs::read(f.data()).unwrap();
    f.run(&["internal", "ensure", "/other"]);
    assert_eq!(before, fs::read(f.data()).unwrap());
    f.run(&["internal", "reset", "/new"]);
    assert!(!f.output(&["internal", "commit", token]).status.success());
    assert_eq!(f.run(&["internal", "list", "back", "10"]).stdout, b"");
    assert_eq!(f.run(&["internal", "list", "forward", "10"]).stdout, b"");
}

/// The file lock must serialize two commits of the same selected token.
#[test]
fn concurrency_only_one_commit_of_the_same_token_succeeds() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    f.run(&["internal", "visit", "--from", "/a", "--to", "/b"]);
    let selection = f.run(&["internal", "select", "back", "1"]);
    let token = std::str::from_utf8(&selection.stdout)
        .unwrap()
        .split_once('\n')
        .unwrap()
        .0;
    let commands = [
        f.command(&["internal", "commit", token]),
        f.command(&["internal", "commit", token]),
    ];
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = commands
        .into_iter()
        .map(|command| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                bounded_output(command)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|o| o.status.success()).count(), 1);
    let loser = results.iter().find(|o| !o.status.success()).unwrap();
    assert!(String::from_utf8_lossy(&loser.stderr).contains("changed since selection"));
    assert!(results.iter().all(|o| o.stdout.is_empty()));
    assert_eq!(f.run(&["internal", "list", "forward", "1"]).stdout, b"/b\n");
}
