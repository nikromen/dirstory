mod cli;
mod config;
mod enums;
mod history;
mod picker;
mod shell;
mod storage;
mod utils;

use clap::Parser;

use cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(error) = cli.run() {
        eprintln!("dirstory: {error}");
        std::process::exit(if error.kind() == std::io::ErrorKind::InvalidInput {
            2
        } else {
            1
        });
    }
}
