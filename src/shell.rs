//! Initialization scripts with tracking adapted to each shell's native hooks.

use crate::enums::Shell;

/// Generate navigation functions and register the shell's tracking mechanism.
///
/// Bash and sh wrap the requested command. Zsh and Fish track directory changes
/// independently of cd; a non-default command remains a convenience function.
pub fn generate_template(shell: &Shell, command: &str) -> String {
    let template = match shell {
        Shell::Sh => include_str!("shell/sh.sh"),
        Shell::Bash => include_str!("shell/bash.bash"),
        Shell::Zsh => include_str!("shell/zsh.zsh"),
        Shell::Fish => include_str!("shell/fish.fish"),
    };

    // Bash and Zsh share navigation, but only Bash reconciles before selecting.
    let sync = match shell {
        Shell::Zsh => "# Explicit hook suppression (cd -q) remains untracked.",
        _ => "_dirstory_sync || return 1",
    };
    let common = include_str!("shell/common.sh").replace("<SYNC>", sync);

    let custom = if command == "cd" {
        String::new()
    } else {
        match shell {
            Shell::Fish => format!("function {command}\n    builtin cd $argv\nend\n"),
            _ => format!("{command}() {{ builtin cd \"$@\"; }}\n"),
        }
    };

    template
        .replace("<COMMON>", &common)
        .replace("<CUSTOM_COMMAND>", &custom)
        .replace("<COMMAND>", command)
}
