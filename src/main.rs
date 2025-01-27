mod cli;
mod config;
mod enums;
mod shell;
mod stack;
mod utils;

use clap::Parser;

use cli::Cli;

fn main() {
    let cli = Cli::parse();
    cli.run();
}
