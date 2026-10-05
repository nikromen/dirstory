mod cli;
mod config;
mod enums;
mod history;
mod shell;
mod storage;
mod utils;

use clap::Parser;

use cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(error) = cli.run() {
        eprintln!("dirstory: {error}");
        std::process::exit(1);
    }
}
