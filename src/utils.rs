use std::env;
use std::process;

use crate::config::{Config, Mode};

pub fn get_tmp_dir() -> String {
    let tmp_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let config = Config::new();
    let identifier = if config.mode == Mode::Tmux {
        env::var("TMUX_PANE")
            .unwrap()
            .trim_start_matches('%')
            .to_string()
    } else {
        // Assumes other scripts are launched from profiles (e.g. .bashrc) to match
        // the parent process id. Otherwise, it needs to find grandparent process id to
        // match the shell process id.
        process::id().to_string()
    };

    format!("{}/dirstory/{}", tmp_dir, identifier)
}
