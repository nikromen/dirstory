use clap::ValueEnum;

#[derive(Clone, Debug, ValueEnum)]
pub enum StackType {
    Backward,
    Forward,
}

impl StackType {
    pub fn as_str(&self) -> &str {
        match self {
            StackType::Backward => "backward",
            StackType::Forward => "forward",
        }
    }
}

#[derive(Clone, Debug, ValueEnum)]
pub enum Shell {
    Sh,
    Bash,
    Zsh,
    Fish,
}
