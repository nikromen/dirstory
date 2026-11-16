use crate::{
    config::Config, enums::Shell, history::History, shell::generate_template, utils::get_tmp_dir,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// Validate an identifier before embedding it in any shell's function syntax.
fn command_name(value: &str) -> Result<String, String> {
    const RESERVED: &[&str] = &[
        "_",
        "if",
        "then",
        "else",
        "elif",
        "fi",
        "case",
        "esac",
        "for",
        "while",
        "until",
        "do",
        "done",
        "in",
        "function",
        "select",
        "time",
        "coproc",
        "repeat",
        "foreach",
        "always",
        "and",
        "or",
        "not",
        "begin",
        "end",
        "switch",
        "break",
        "continue",
        "builtin",
        "command",
        "local",
        "set",
        "printf",
        "test",
        "return",
        "eval",
        "echo",
        "true",
        "false",
        "source",
        "exec",
        "declare",
        "typeset",
        "export",
        "readonly",
        "unset",
        "read",
        "argparse",
        "string",
        "math",
        "count",
        "status",
        "functions",
        "b",
        "f",
        "dirstory",
    ];
    let mut bytes = value.bytes();
    let valid = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err("Use an ASCII identifier: [A-Za-z_][A-Za-z0-9_]*".into());
    }
    if RESERVED.contains(&value) || value.starts_with("_dirstory_") {
        return Err("Command name is reserved or conflicts with dirstory".into());
    }
    Ok(value.into())
}

/// Parsed selection identity; reject malformed tokens before opening history.
#[derive(Clone, Debug)]
struct CommitToken {
    revision: u64,
    offset: u64,
}

fn commit_token(value: &str) -> Result<CommitToken, String> {
    let parse = || {
        let (revision, offset) = value.split_once(':')?;
        if [revision, offset]
            .iter()
            .any(|v| v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        Some(CommitToken {
            revision: revision.parse().ok()?,
            offset: offset.parse().ok()?,
        })
    };
    parse().ok_or_else(|| "Invalid selection token; expected REVISION:OFFSET".into())
}

#[derive(Debug, Parser)]
#[command(
    about = "Navigate backward and forward through visited directories",
    author,
    version
)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Private protocol used by shell adapters; compatibility is not guaranteed.
    #[command(hide = true)]
    Internal {
        #[command(subcommand)]
        cmd: InternalCommand,
    },

    /// Print the initialization script for a supported shell.
    #[command(
        long_about = "Print shell functions for tracking directory changes and navigating with b/f.

Load the output in your shell configuration: eval \"$(dirstory init bash)\" for Bash,
eval \"$(dirstory init zsh)\" for Zsh, or dirstory init fish | source for Fish."
    )]
    Init {
        /// Directory-change command: wrapper in Bash/sh, convenience in Zsh/Fish.
        #[arg(short, long, default_value = "cd", value_name = "COMMAND", value_parser = command_name)]
        command: String,

        /// Shell for which to generate initialization code.
        #[arg(value_name = "SHELL")]
        shell: Shell,
    },
}

/// Direction relative to the current history position.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Direction {
    /// Earlier directory visits.
    Back,

    /// Later directory visits.
    Forward,
}

impl Direction {
    /// Whether traversal should follow earlier visits.
    fn back(self) -> bool {
        matches!(self, Self::Back)
    }
}

#[derive(Debug, Subcommand)]
enum InternalCommand {
    /// Print every visit, oldest first, without changing history.
    #[command(long_about = "Capture all visits under one shared lock, oldest first.

Each visit is three LF-terminated lines: REVISION:OFFSET, current flag (1 or 0),
and the unchanged Unix path bytes. Exactly one visit is current; repeated paths
have distinct tokens. The entire snapshot is validated before output, including
rejecting LF in paths. Invalid or uninitialized history produces no protocol output.

Keep the chosen token and path together. After successful cd to that path, pass
the token to internal commit. Do not shell-escape or eval path bytes. The snapshot
lock is released before output; user selection must not retain a history lock.")]
    Snapshot,

    /// Select a navigation target without changing the current history position.
    #[command(long_about = "Select a navigation target without changing history.

Print REVISION:OFFSET on the first line and the target path on the second.
The shell must perform cd successfully before passing the token to internal commit.

Excess steps stop at the boundary; zero selects the current visit.
An unavailable direction produces no output. Paths containing LF are unsupported.")]
    Select {
        /// Direction in which to navigate.
        direction: Direction,

        /// Number of visits to move through.
        #[arg(default_value = "1", value_name = "N")]
        n: usize,
    },

    /// Initialize missing history, leaving existing valid history unchanged.
    Ensure {
        /// Current directory used for the initial visit.
        path: PathBuf,
    },

    /// Discard history and replace it with a single visit.
    #[command(long_about = "Discard all visits and initialize history at PATH.

This also repairs invalid or interrupted history; discarded visits cannot be recovered.")]
    Reset {
        /// Directory that becomes the only visit and current position.
        path: PathBuf,
    },

    /// Record a successful directory change and discard the forward branch.
    #[command(
        long_about = "Record a directory change after the shell has successfully performed cd.

If the stored current directory differs from --from, restart history at --from.
An unchanged directory adds no visit; a new visit discards the forward branch."
    )]
    Visit {
        /// Working directory before the successful cd.
        #[arg(long, value_name = "PATH")]
        from: PathBuf,

        /// Working directory after the successful cd.
        #[arg(long, value_name = "PATH")]
        to: PathBuf,
    },

    /// Print up to N visits in the chosen direction, nearest first.
    #[command(
        long_about = "Print up to N neighboring visits, one path per line, nearest first.

The current visit is excluded. Listing does not modify history."
    )]
    List {
        /// Side of the current position to list.
        direction: Direction,

        /// Maximum number of visits to print; zero prints nothing.
        #[arg(value_name = "N")]
        n: usize,
    },

    /// Confirm a selection after the shell has successfully changed directory.
    #[command(
        long_about = "Move the history position to a previously selected visit.

TOKEN must be REVISION:OFFSET returned by internal select or internal snapshot.
A stale token is rejected; this command does not undo a cd already performed by the shell."
    )]
    Commit {
        /// Visit token returned by internal select or internal snapshot.
        #[arg(value_name = "TOKEN", value_parser = commit_token)]
        token: CommitToken,
    },
}

/// Reject paths that cannot be represented by the line-based shell protocol.
fn check_path(path: &std::path::Path) -> io::Result<()> {
    if path.as_os_str().as_bytes().contains(&b'\n') {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Paths containing LF are not supported by the shell protocol",
        ))
    } else {
        Ok(())
    }
}

fn print_path(path: &std::path::Path) -> io::Result<()> {
    check_path(path)?;
    let mut out = io::stdout().lock();
    out.write_all(path.as_os_str().as_bytes())?;
    out.write_all(b"\n")
}

impl Cli {
    pub fn run(&self) -> io::Result<()> {
        if let Commands::Init { command, shell } = &self.cmd {
            println!("{}", generate_template(shell, command));
            return Ok(());
        }

        let Commands::Internal { cmd } = &self.cmd else {
            unreachable!();
        };

        match cmd {
            InternalCommand::Ensure { path } | InternalCommand::Reset { path } => check_path(path)?,
            InternalCommand::Visit { from, to } => {
                check_path(from)?;
                check_path(to)?;
            }
            _ => {}
        }

        let config = Config::new()?;
        let dir = get_tmp_dir(config.mode)?;
        let exclusive = !matches!(
            cmd,
            InternalCommand::Select { .. }
                | InternalCommand::List { .. }
                | InternalCommand::Snapshot
        );
        let mut h = History::open(&dir, exclusive)?;

        match cmd {
            InternalCommand::Snapshot => {
                let snapshot = h.snapshot()?;
                for entry in &snapshot.entries {
                    check_path(&entry.path)?;
                }
                drop(h);
                let mut out = io::stdout().lock();
                for entry in snapshot.entries {
                    writeln!(out, "{}:{}", snapshot.revision, entry.offset)?;
                    writeln!(out, "{}", u8::from(entry.offset == snapshot.current))?;
                    out.write_all(entry.path.as_os_str().as_bytes())?;
                    out.write_all(b"\n")?;
                }
            }
            InternalCommand::Select { direction, n } => {
                if let Some(s) = h.select(direction.back(), *n)? {
                    check_path(&s.entry.path)?;
                    println!("{}:{}", s.revision, s.entry.offset);
                    print_path(&s.entry.path)?;
                }
            }
            InternalCommand::Ensure { path } => {
                h.ensure(path)?;
            }
            InternalCommand::Reset { path } => {
                h.reset(path)?;
            }
            InternalCommand::Visit { from, to } => {
                h.visit(from, to)?;
            }
            InternalCommand::List { direction, n } => {
                let entries = h.list(direction.back(), *n)?;

                for e in &entries {
                    check_path(&e.path)?;
                }

                for e in entries {
                    print_path(&e.path)?;
                }
            }
            InternalCommand::Commit { token } => {
                h.commit(token.revision, token.offset)?;
            }
        }

        Ok(())
    }
}
