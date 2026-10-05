use clap::ValueEnum;

/// Shell dialect used when generating initialization functions.
#[derive(Clone, Debug, ValueEnum)]
pub enum Shell {
    Sh,
    Bash,
    Zsh,
    Fish,
}
