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
    let arguments = match shell {
        Shell::Fish => include_str!("shell/arguments.fish"),
        _ => include_str!("shell/arguments.sh"),
    };
    let help = "Usage: $dirstory_name [N]
       $dirstory_name -l|--list [N]
       $dirstory_name -i|--interactive
       $dirstory_name -h|--help

Navigate through directory visits (default: 1 step).
  -i, --interactive  Open full directory history (also: dh).
  -l, --list  List neighboring visits (default: 10).
  -h, --help  Show this help.
N must be a nonnegative decimal integer; zero is allowed.";

    let dh_help = "Usage: dh [--reset | -h | --help]

Open interactive directory history.
  --reset  Discard all visits and restart at the current directory.
           Use after invalid history or an interrupted write.
           Removed visits cannot be recovered; affects all shells sharing this history.
  -h, --help  Show this help.";

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
        .replace("<ARGUMENTS>", arguments)
        .replace("<HELP>", help)
        .replace("<DH_HELP>", dh_help)
        .replace("<MAX_COUNT>", &usize::MAX.to_string())
        .replace("<CUSTOM_COMMAND>", &custom)
        .replace("<COMMAND>", command)
}
