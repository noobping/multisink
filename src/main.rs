mod audio;
mod cli;
mod gui;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            if let Err(e) = cli::handle_command(cmd) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            // No subcommand -> show a tiny usage hint
            println!("multisink – multi-output audio helper");
            println!();
            println!("Usage:");
            println!("  multisink list");
            println!("  multisink enable [--sinks=NAME,NAME,...]");
            println!("  multisink disable");
            println!("  multisink gui");
        }
    }
}
