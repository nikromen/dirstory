use std::env;
use std::fs;
use std::os::unix::process;
use std::process::id;

use crate::config::Mode;

pub fn get_tmp_dir(mode: Mode) -> String {
    let tmp_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let identifier = match mode {
        Mode::Tmux => env::var("TMUX_PANE")
            .unwrap()
            .trim_start_matches('%')
            .to_string(),
        // Assumes other scripts are launched from profiles (e.g. .bashrc) to match
        // the parent process id. Otherwise, it needs to find grandparent process id to
        // match the shell process id.
        Mode::Ppid => process::parent_id().to_string(),
        Mode::Tty => {
            let tty_path = fs::read_link(format!("/proc/{}/fd/0", id())).unwrap();
            let tty = tty_path.to_string_lossy();
            if let Some(pos) = tty.rfind('/') {
                tty[pos + 1..].to_string()
            } else {
                panic!("Failed to get tty name");
            }
        }
    };

    format!("{}/dirstory/{}", tmp_dir, identifier)
}
