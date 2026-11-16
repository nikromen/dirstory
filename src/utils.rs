use std::{env, fs, io, path::PathBuf, process::id};

use crate::config::Mode;

/// Resolve a session directory; missing environment or proc entries are errors.
pub fn get_tmp_dir(mode: Mode) -> io::Result<PathBuf> {
    let tmp_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    let identifier = match mode {
        Mode::Tmux => {
            let pane = env::var("TMUX_PANE").map_err(|error| {
                io::Error::other(format!(
                    "Cannot use tmux mode: TMUX_PANE is unavailable: {error}"
                ))
            })?;
            let identifier = pane.trim_start_matches('%');
            if identifier.is_empty() || identifier.contains('/') || matches!(identifier, "." | "..")
            {
                return Err(io::Error::other("Cannot use tmux mode: invalid TMUX_PANE"));
            }
            identifier.to_owned().into()
        }
        Mode::Pid => {
            let pid = env::var("DIRSTORY_SHELL_PID").map_err(|_| {
                io::Error::other(
                    "Cannot use pid mode: DIRSTORY_SHELL_PID is unavailable; initialize your shell",
                )
            })?;
            if pid.is_empty()
                || !pid.bytes().all(|byte| byte.is_ascii_digit())
                || pid.parse::<u32>().ok().filter(|&pid| pid > 0).is_none()
            {
                return Err(io::Error::other(
                    "Cannot use pid mode: invalid DIRSTORY_SHELL_PID",
                ));
            }
            pid.into()
        }
        Mode::Tty => {
            let tty_path = fs::read_link(format!("/proc/{}/fd/0", id())).map_err(|error| {
                io::Error::new(error.kind(), format!("Cannot identify terminal: {error}"))
            })?;
            // A substitution may provide a pipe instead of a terminal path.
            // Preserve the existing session contract without creating pipe sessions.
            if !tty_path.is_absolute() {
                return Err(io::Error::other(
                    "Cannot identify terminal from standard input",
                ));
            }
            tty_path
                .file_name()
                .ok_or_else(|| io::Error::other("Cannot identify terminal name"))?
                .to_owned()
        }
    };
    Ok(tmp_dir.join("dirstory").join(identifier))
}
