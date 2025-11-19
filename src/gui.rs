use crate::audio;
use crate::audio::Sink;
use eframe::egui;

struct SinkUi {
    sink: Sink,
    selected: bool,
}

pub struct MultisinkApp {
    sinks: Vec<SinkUi>,
    status: String,
    backend_ok: bool,
}

impl MultisinkApp {
    pub fn new() -> Self {
        let mut app = Self {
            sinks: Vec::new(),
            status: String::new(),
            backend_ok: true,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match audio::check_backend() {
            Ok(()) => {
                self.backend_ok = true;
                match audio::list_sinks() {
                    Ok(sinks) => {
                        let combined_present = sinks
                            .iter()
                            .any(|s| s.is_combined);

                        self.sinks = sinks
                            .into_iter()
                            .map(|s| {
                                let default_selected = !s.is_combined;
                                SinkUi {
                                    sink: s,
                                    selected: default_selected,
                                }
                            })
                            .collect();

                        self.status = if combined_present {
                            format!(
                                "Backend OK. Combined sink '{}' is present.",
                                audio::COMBINED_SINK_NAME
                            )
                        } else {
                            "Backend OK. No combined sink yet.".to_string()
                        };
                    }
                    Err(e) => {
                        self.sinks.clear();
                        self.status = format!("Error listing sinks: {e}");
                    }
                }
            }
            Err(e) => {
                self.backend_ok = false;
                self.sinks.clear();
                self.status = format!("Audio backend not available: {e}");
            }
        }
    }
}

impl eframe::App for MultisinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Multisink – multi-output audio");
            ui.label(&self.status);

            if !self.backend_ok {
                ui.colored_label(
                    egui::Color32::RED,
                    "pactl / PulseAudio/PipeWire backend not detected.\n\
                     Make sure PulseAudio or PipeWire with pipewire-pulse is running.",
                );
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }
                return;
            }

            ui.separator();
            ui.label("Select which sinks to include in the combined output:");

            egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    for s in &mut self.sinks {
                        let label = if s.sink.is_combined {
                            format!("{} (#{}) [combined]", s.sink.name, s.sink.index)
                        } else {
                            format!("{} (#{})", s.sink.name, s.sink.index)
                        };

                        // Don't allow toggling the combined sink itself
                        if s.sink.is_combined {
                            ui.checkbox(&mut false, label)
                                .on_hover_text("This is the combined sink itself.");
                        } else {
                            ui.checkbox(&mut s.selected, label);
                        }
                    }
                });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }

                if ui.button("Enable combined output").clicked() {
                    let selected_names: Vec<String> = self
                        .sinks
                        .iter()
                        .filter(|s| s.selected && !s.sink.is_combined)
                        .map(|s| s.sink.name.clone())
                        .collect();

                    let res = if selected_names.is_empty() {
                        audio::enable_combined(None)
                    } else {
                        audio::enable_combined(Some(&selected_names))
                    };

                    match res {
                        Ok(()) => {
                            self.status = format!(
                                "Enabled combined sink '{}'.",
                                audio::COMBINED_SINK_NAME
                            );
                            self.refresh();
                        }
                        Err(e) => {
                            self.status = format!("Failed to enable combined sink: {e}");
                        }
                    }
                }

                if ui.button("Disable combined output").clicked() {
                    match audio::disable_combined() {
                        Ok(()) => {
                            self.status =
                                format!("Disabled combined sink '{}'.", audio::COMBINED_SINK_NAME);
                            self.refresh();
                        }
                        Err(e) => {
                            self.status = format!("Failed to disable combined sink: {e}");
                        }
                    }
                }
            });
        });
    }
}

