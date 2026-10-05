use std::{
    fs,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
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
        let o = self.command(args).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        o
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
    assert!(!f
        .command(&["internal", "commit", token])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn writer_waits_for_lock() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.data().with_file_name("history.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut child = f
        .command(&["internal", "visit", "--from", "/a", "--to", "/b"])
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(child.try_wait().unwrap().is_none());
    lock.unlock().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(f.run(&["internal", "list", "back", "1"]).stdout, b"/a\n");
}

#[test]
fn interrupted_write_is_rejected_and_reset_recovers() {
    let f = Fixture::new();
    f.run(&["internal", "ensure", "/a"]);
    let mut file = OpenOptions::new().write(true).open(f.data()).unwrap();
    file.seek(SeekFrom::Start(16)).unwrap();
    file.write_all(&1u64.to_le_bytes()).unwrap();
    let result = f
        .command(&["internal", "select", "back", "1"])
        .output()
        .unwrap();
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
        .command(&["internal", "visit", "--from", "/a", "--to", "/b\nc"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(before, fs::read(f.data()).unwrap());
    let result = f
        .command(&["internal", "ensure", "/other"])
        .env("TMUX_PANE", "%other")
        .output()
        .unwrap();
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
        assert!(!f
            .command(&[old, "--help"])
            .output()
            .unwrap()
            .status
            .success());
    }
}
