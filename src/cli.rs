use std::env;

use clap::{Parser, Subcommand};

use crate::enums::{Shell, StackType};
use crate::shell::generate_template;
use crate::stack::get_or_create_stack_from_path;
use crate::utils::get_tmp_dir;

#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Operate on a stack")]
    Stack {
        #[arg(short, long, help = "Stack type to operate on")]
        stack_type: StackType,
        #[command(subcommand)]
        stack_cmd: StackCommands,
    },
    #[command(about = "Navigate backward or forward in the history")]
    Navigate {
        #[command(subcommand)]
        navigate_cmd: NavigateCommands,
    },
    #[command(about = "Initialize the shell script")]
    Init {
        #[arg(
            short,
            long,
            default_value = "cd",
            help = "Name of the command wrapper around cd"
        )]
        command: String,
        #[arg(name = "SHELL", help = "Shell to initialize")]
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
enum StackCommands {
    #[command(about = "Push a directory onto the stack")]
    Push {
        #[arg(name = "DIR", help = "Directory to push")]
        dir: String,
    },
    #[command(about = "Pop directories from the stack")]
    Pop {
        #[arg(
            name = "N",
            default_value = "1",
            help = "Number of directories to pop and print to stdout"
        )]
        n: usize,
    },
    #[command(about = "List directories in the stack")]
    List {
        #[arg(name = "N", help = "Number of directories to list")]
        n: usize,
    },
    #[command(about = "Empty the stack")]
    Empty,
}

#[derive(Debug, Subcommand)]
enum NavigateCommands {
    #[command(about = "Go back in the history and print the current directory")]
    Back {
        #[arg(
            name = "N",
            default_value = "1",
            help = "Number of directories to go back"
        )]
        n: usize,
    },
    #[command(about = "Go forward in the history and print the current directory")]
    Forward {
        #[arg(
            name = "N",
            default_value = "1",
            help = "Number of directories to go forward"
        )]
        n: usize,
    },
}

fn match_navigate_commands(navigate_cmd: &NavigateCommands) {
    let prefix = get_tmp_dir() + "/";
    let current_dir = env::current_dir().unwrap();
    let pwd = current_dir.to_str().unwrap().to_string();

    let back_stack_path = prefix.clone() + StackType::Backward.as_str();
    let mut backward_stack = get_or_create_stack_from_path(&back_stack_path);

    let forward_stack_path = prefix + StackType::Forward.as_str();
    let mut forward_stack = get_or_create_stack_from_path(&forward_stack_path);

    match navigate_cmd {
        NavigateCommands::Back { n } => {
            let popped = backward_stack.pop(*n);
            if popped.is_empty() {
                return;
            }

            forward_stack.push(&pwd);
            for dir in &popped {
                if dir == popped.last().unwrap() {
                    println!("{}", dir);
                    return;
                }
                forward_stack.push(dir);
            }
        }
        NavigateCommands::Forward { n } => {
            let popped = forward_stack.pop(*n);
            if popped.is_empty() {
                return;
            }

            backward_stack.push(&pwd);
            for dir in &popped {
                if dir == popped.last().unwrap() {
                    println!("{}", dir);
                    return;
                }
                backward_stack.push(dir);
            }
        }
    }
}

fn match_stack_commands(stack_cmd: &StackCommands, stack_type: &StackType) {
    let stack_path = get_tmp_dir() + "/" + stack_type.as_str();
    let mut stack = get_or_create_stack_from_path(&stack_path);

    match stack_cmd {
        StackCommands::Push { dir } => {
            stack.push(dir);
        }
        StackCommands::Pop { n } => {
            let popped = stack.pop(*n);
            for dir in &popped {
                println!("{}", dir);
            }
        }
        StackCommands::List { n } => {
            let dirs = stack.get_n(*n);
            for dir in &dirs {
                println!("{}", dir);
            }
        }
        StackCommands::Empty => {
            stack.empty().unwrap();
        }
    }
}

impl Cli {
    pub fn run(&self) {
        match &self.cmd {
            Commands::Stack {
                stack_type,
                stack_cmd,
            } => {
                match_stack_commands(stack_cmd, stack_type);
            }
            Commands::Navigate { navigate_cmd } => {
                match_navigate_commands(navigate_cmd);
            }
            Commands::Init { command, shell } => {
                let template = generate_template(shell, command);
                println!("{}", template);
            }
        }
    }
}
