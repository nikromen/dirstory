use std::{
    env, fs,
    io::{self, IsTerminal},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    process::id,
};

use crate::config::Mode;

/// Resolve a session directory; missing environment or proc entries are errors.
pub fn get_tmp_dir(mode: Mode) -> io::Result<PathBuf> {
    let tmp_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    let identifier: String = match mode {
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
            format!("tmux-{identifier}")
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
            format!("pid-{pid}")
        }
        Mode::Tty => {
            if !io::stdin().is_terminal() {
                return Err(io::Error::other(
                    "Cannot identify terminal from standard input",
                ));
            }
            let tty_path = fs::read_link(format!("/proc/{}/fd/0", id())).map_err(|error| {
                io::Error::new(error.kind(), format!("Cannot identify terminal: {error}"))
            })?;
            tty_identifier(&tty_path)
        }
    };
    Ok(tmp_dir.join("dirstory").join(identifier))
}

// Hex encodes every byte, preserving the full device path without separators.
fn tty_identifier(path: &std::path::Path) -> String {
    use std::fmt::Write;
    let mut identifier = String::from("tty-");
    for byte in path.as_os_str().as_bytes() {
        write!(identifier, "{byte:02x}").unwrap();
    }
    identifier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_names_preserve_the_full_device_path() {
        assert_ne!(
            tty_identifier("/dev/tty3".as_ref()),
            tty_identifier("/dev/pts/3".as_ref())
        );
        assert_eq!(
            tty_identifier("/dev/pts/3".as_ref()),
            "tty-2f6465762f7074732f33"
        );
    }
}
