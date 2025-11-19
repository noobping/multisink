use crate::audio;
use crate::audio::AudioError;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "multisink",
    about = "Create a combined audio sink across multiple outputs",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Enable combined output (optionally only on selected sinks)
    Enable {
        /// Comma-separated sink names to use (default: all physical sinks)
        #[arg(long, value_delimiter = ',', num_args = 0..)]
        sinks: Vec<String>,
    },

    /// Disable the combined sink if present
    Disable,

    /// List sinks and whether the combined sink exists
    List,

    /// Launch the GUI
    Gui,
}

pub fn handle_command(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::List => {
            audio::check_backend()?;
            let sinks = audio::list_sinks()?;
            println!("Available sinks:");

            for s in sinks {
                let marker = if s.is_combined { "[combined]" } else { "          " };
                println!("{marker} #{}  {}", s.index, s.name);
            }

            let combined = audio::combined_sink_exists()?;
            println!(
                "\nCombined sink '{}' present: {}",
                audio::COMBINED_SINK_NAME,
                if combined { "yes" } else { "no" }
            );
        }

        Command::Enable { sinks } => {
            audio::check_backend()?;
            if sinks.is_empty() {
                println!("Enabling combined sink using all physical sinks…");
                match audio::enable_combined(None) {
                    Ok(()) => println!(
                        "Combined sink '{}' enabled and set as default.",
                        audio::COMBINED_SINK_NAME
                    ),
                    Err(AudioError::NotEnoughSinks) => {
                        println!("Not enough sinks - need at least 2 outputs to combine.");
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                println!("Enabling combined sink using sinks: {}", sinks.join(", "));
                match audio::enable_combined(Some(&sinks)) {
                    Ok(()) => println!(
                        "Combined sink '{}' enabled and set as default.",
                        audio::COMBINED_SINK_NAME
                    ),
                    Err(e) => return Err(e.into()),
                }
            }
        }

        Command::Disable => {
            audio::check_backend()?;
            match audio::disable_combined() {
                Ok(()) => println!("Combined sink '{}' disabled.", audio::COMBINED_SINK_NAME),
                Err(AudioError::CombinedNotFound) => {
                    println!("Combined sink not found; nothing to disable.");
                }
                Err(e) => return Err(e.into()),
            }
        }

        Command::Gui => {
            let native_options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size(eframe::egui::vec2(480.0, 360.0))
                    .with_min_inner_size(eframe::egui::vec2(385.0, 200.0))
                    .with_resizable(true),
                ..Default::default()
            };

            if let Err(e) = eframe::run_native(
                "Multisink",
                native_options,
                Box::new(|_cc| {
                    Ok::<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>>(
                        Box::new(crate::gui::MultisinkApp::new()),
                    )
                }),
            ) {
                eprintln!("GUI error: {e}");
            }
        }
    }

    Ok(())
}
