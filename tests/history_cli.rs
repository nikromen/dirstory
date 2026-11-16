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
            r#"{"mode":"tmux"}"#,
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

/// Names embedded in generated code must be portable and collision-free.
#[test]
fn initialization_validates_names_before_emitting_code() {
    let f = Fixture::new();
    for shell in ["bash", "zsh", "fish", "sh"] {
        for name in ["cd", "cdir", "_jump", "Jump42"] {
            assert!(!f.run(&["init", shell, "--command", name]).stdout.is_empty());
        }
        for name in [
            "",
            "1cd",
            "c-d",
            "á",
            "cd;touch /tmp/injected",
            "$(id)",
            "cd\nfalse",
            "if",
            "function",
            "b",
            "f",
            "dirstory",
            "_dirstory_sync",
            "command",
            "builtin",
            "set",
            "printf",
            "string",
        ] {
            let result = f.output(&["init", shell, "--command", name]);
            assert_eq!(result.status.code(), Some(2), "{shell}: {name:?}");
            assert!(result.stdout.is_empty());
            assert!(!result.stderr.is_empty());
        }
    }
    assert!(!f.data().exists());
}

/// Invalid protocol inputs are rejected before opening or creating storage.
#[test]
fn invalid_protocol_inputs_do_not_create_history() {
    let f = Fixture::new();
    for args in [
        vec!["internal", "commit", "1:+64"],
        vec!["internal", "commit", "1:18446744073709551616"],
        vec!["internal", "ensure", "/bad\npath"],
        vec!["internal", "visit", "--from", "/a", "--to", "/bad\npath"],
    ] {
        assert_eq!(f.output(&args).status.code(), Some(2));
        assert!(!f.root.path().join("dirstory").exists());
    }
}

/// Configuration errors have a location and a runtime exit code, never a panic.
#[test]
fn configuration_errors_are_contextual_and_do_not_fall_back() {
    let f = Fixture::new();
    let yaml = f.root.path().join("config/dirstory.yaml");
    for content in [
        "mode: invalid",
        "mode: [",
        "unique_top: true",
        "unknown: 1",
        "mode: 42",
    ] {
        fs::write(&yaml, content).unwrap();
        let result = f.output(&["internal", "ensure", "/a"]);
        assert_eq!(result.status.code(), Some(1));
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("dirstory.yaml"), "{error}");
        assert!(!error.contains("panicked"), "{error}");
        assert!(result.stdout.is_empty());
        assert!(!f.data().exists());
    }
    fs::write(&yaml, "mode: pid").unwrap();
    let mut command = f.command(&["internal", "ensure", "/a"]);
    command.env("DIRSTORY_SHELL_PID", "12345");
    assert!(bounded_output(command).status.success());
    assert!(!f.data().exists(), "YAML must take precedence over JSON");
}

#[test]
fn missing_session_and_configuration_environment_returns_errors() {
    let f = Fixture::new();
    let mut command = f.command(&["internal", "ensure", "/a"]);
    command.env_remove("TMUX_PANE");
    let result = bounded_output(command);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("TMUX_PANE"));
    for pane in ["", "%", "../escape", "/escape"] {
        let mut command = f.command(&["internal", "ensure", "/a"]);
        command.env("TMUX_PANE", pane);
        assert_eq!(bounded_output(command).status.code(), Some(1));
    }
    let mut command = f.command(&["internal", "ensure", "/a"]);
    command.env_remove("HOME").env("XDG_CONFIG_HOME", "");
    let result = bounded_output(command);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("configuration"));
    assert!(!f.data().exists());
}

#[test]
fn pid_mode_requires_a_valid_shell_pid() {
    let f = Fixture::new();
    fs::write(f.root.path().join("config/dirstory.yaml"), "mode: pid").unwrap();
    for pid in [
        None,
        Some(""),
        Some("0"),
        Some("../escape"),
        Some("4294967296"),
    ] {
        let mut command = f.command(&["internal", "ensure", "/a"]);
        match pid {
            Some(pid) => {
                command.env("DIRSTORY_SHELL_PID", pid);
            }
            None => {
                command.env_remove("DIRSTORY_SHELL_PID");
            }
        }
        let output = bounded_output(command);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("DIRSTORY_SHELL_PID"));
    }
    assert!(!f.root.path().join("dirstory").exists());
}

#[test]
fn config_home_fallback_and_non_utf8_paths_work() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let home = f.root.path().join("home");
    fs::create_dir_all(home.join(".config")).unwrap();
    fs::write(home.join(".config/dirstory.json"), r#"{"mode":"tmux"}"#).unwrap();
    let mut command = f.command(&["internal", "ensure", "/a"]);
    command.env("HOME", home).env("XDG_CONFIG_HOME", "");
    assert!(bounded_output(command).status.success());
    let dir = f
        .root
        .path()
        .join(std::ffi::OsString::from_vec(b"config-\xff".to_vec()));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("dirstory.json"), r#"{"mode":"tmux"}"#).unwrap();
    let mut command = f.command(&["internal", "ensure", "/a"]);
    command.env("XDG_CONFIG_HOME", dir);
    assert!(bounded_output(command).status.success());
}
