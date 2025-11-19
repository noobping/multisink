mod audio;
mod cli;
mod gui;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        // CLI mode
        if let Err(e) = cli::handle_command(cmd) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    } else {
        // GUI mode
        let native_options = eframe::NativeOptions::default();

        // eframe::run_native returns Result<_, eframe::Error>, and the closure is now expected
        // to return Result<Box<dyn App>, Box<dyn Error + Send + Sync>>.
        if let Err(e) = eframe::run_native(
            "Multisink",
            native_options,
            Box::new(|_cc| {
                // App creator closure:
                Ok::<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>>(
                    Box::new(gui::MultisinkApp::new()),
                )
            }),
        ) {
            eprintln!("GUI error: {e}");
            std::process::exit(1);
        }
    }
}
