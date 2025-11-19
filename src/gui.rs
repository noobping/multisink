use crate::audio;
use crate::audio::Sink;
use gtk4::prelude::*;
use adw::{ApplicationWindow, WindowTitle};
use gtk4::{
    gio, Application, Builder, Box as GtkBox, Button, CheckButton, Label,
    ListBox,
};

const APP_ID: &str = "dev.noobping.multisink";
const UI_SRC: &str = include_str!("../data/multisink.ui");

pub fn run_gui() -> anyhow::Result<()> {
    // Register compiled GResources (from build.rs)
    gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    // If you want to *force* Wayland:
    // std::env::set_var("GDK_BACKEND", "wayland");

    let app = Application::builder()
        .application_id(APP_ID)
        // We handle command line ourselves so GLib doesn't complain
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    // Ignore CLI args like "gui" and just activate
    app.connect_command_line(|app, _cmd| {
        app.activate();
        0.into()
    });

    app.connect_activate(build_ui);

    if let Ok(backend) = std::env::var("GDK_BACKEND") {
        eprintln!("GDK_BACKEND = {backend}");
    }

    app.run();
    Ok(())
}

fn build_ui(app: &Application) {
    let builder = Builder::from_string(UI_SRC);

    let window: ApplicationWindow = builder
        .object("main_window")
        .expect("Failed to get main_window from UI");
    window.set_application(Some(app));

    let window_title: WindowTitle = builder
        .object("window_title")
        .expect("Failed to get window_title");
    let list_box: ListBox = builder
        .object("sink_list")
        .expect("Failed to get sink_list");
    let refresh_button: Button = builder
        .object("refresh_button")
        .expect("Failed to get refresh_button");
    let toggle_button: Button = builder
        .object("toggle_button")
        .expect("Failed to get toggle_button");

    // Refresh button: manual refresh
    {
        let list_box_clone = list_box.clone();
        let status_clone = window_title.clone();
        let toggle_clone = toggle_button.clone();
        refresh_button.connect_clicked(move |_| {
            refresh_sinks(&list_box_clone, &status_clone, &toggle_clone);
        });
    }

    // Toggle button: Enable or Disable based on current state
    {
        let list_box_clone = list_box.clone();
        let status_clone = window_title.clone();
        let toggle_clone = toggle_button.clone();
        toggle_button.connect_clicked(move |_| {
            // Decide what to do based on whether combined sink exists
            let combined_present = audio::combined_sink_exists().unwrap_or(false);

            if combined_present {
                // Disable path
                match audio::disable_combined() {
                    Ok(()) => status_clone.set_subtitle("Combined output disabled."),
                    Err(e) => status_clone.set_subtitle(&format!("Failed to disable combined sink: {e}")),
                }
            } else {
                // Enable path based on current selection
                enable_from_selection(&list_box_clone, &status_clone);
            }

            // Always refresh afterwards to reflect new state
            refresh_sinks(&list_box_clone, &status_clone, &toggle_clone);
        });
    }

    // Auto-refresh when window becomes active (focus in)
    {
        let list_box_clone = list_box.clone();
        let status_clone = window_title.clone();
        let toggle_clone = toggle_button.clone();

        window.connect_is_active_notify(move |win| {
            if win.is_active() {
                refresh_sinks(&list_box_clone, &status_clone, &toggle_clone);
            }
        });
    }

    // Initial populate
    refresh_sinks(&list_box, &window_title, &toggle_button);

    window.show();
}

fn clear_list_box(list_box: &ListBox) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
}

fn refresh_sinks(list_box: &ListBox, title: &WindowTitle, toggle_button: &Button) {
    clear_list_box(list_box);

    if let Err(e) = audio::check_backend() {
        title.set_subtitle(&format!("Audio backend not available: {e}"));
        toggle_button.set_sensitive(false);
        toggle_button.set_tooltip_text(Some("Unavailable"));
        toggle_button.set_icon_name("circle-crossed-symbolic");
        toggle_button.remove_css_class("suggested-action");
        toggle_button.remove_css_class("destructive-action");
        return;
    }

    let sinks: Vec<Sink> = match audio::list_sinks() {
        Ok(s) => s,
        Err(e) => {
            title.set_subtitle(&format!("Error listing sinks: {e}"));
            toggle_button.set_sensitive(false);
            toggle_button.set_tooltip_text(Some("Unavailable"));
            toggle_button.set_icon_name("circle-crossed-symbolic");
            toggle_button.remove_css_class("suggested-action");
            toggle_button.remove_css_class("destructive-action");
            return;
        }
    };

    if sinks.is_empty() {
        title.set_subtitle("No audio outputs found.");
        toggle_button.set_sensitive(false);
        toggle_button.set_tooltip_text(Some("Enable"));
        toggle_button.set_icon_name("checkmark-small-symbolic");
        toggle_button.remove_css_class("destructive-action");
        toggle_button.add_css_class("suggested-action");
        return;
    }

    let combined_present = sinks.iter().any(|s| s.is_combined);

    for s in sinks {
        let row_box = GtkBox::new(gtk4::Orientation::Horizontal, 6);

        let check = CheckButton::new();
        // Store sink name in widget_name to retrieve later
        check.set_widget_name(&s.name);
        // Default: select all non-combined sinks
        check.set_active(!s.is_combined);

        let label_text = if s.is_combined {
            format!("{} (#{} ) [combined]", s.name, s.index)
        } else {
            s.pretty_name
        };
        let label = Label::new(Some(&label_text));
        label.set_xalign(0.0);

        row_box.append(&check);
        row_box.append(&label);

        list_box.append(&row_box);
    }

    // Update status + toggle button style according to state
    if combined_present {
        title.set_subtitle("Enabled combined output");
        toggle_button.set_sensitive(true);
        toggle_button.set_tooltip_text(Some("Disable"));
        toggle_button.set_icon_name("cross-small-symbolic");
        toggle_button.remove_css_class("suggested-action");
        toggle_button.add_css_class("destructive-action");
    } else {
        title.set_subtitle("Disabled combined output");
        toggle_button.set_sensitive(true);
        toggle_button.set_tooltip_text(Some("Enable"));
        toggle_button.set_icon_name("checkmark-small-symbolic");
        toggle_button.remove_css_class("destructive-action");
        toggle_button.add_css_class("suggested-action");
    }
}

fn enable_from_selection(list_box: &ListBox, title: &WindowTitle) {
    let mut selected_names: Vec<String> = Vec::new();

    let mut child_opt = list_box.first_child();
    while let Some(child) = child_opt {
        let next = child.next_sibling();

        if let Ok(row) = child.downcast::<gtk4::ListBoxRow>() {
            if let Some(row_child) = row.child() {
                if let Ok(row_box) = row_child.downcast::<GtkBox>() {
                    if let Some(first_child) = row_box.first_child() {
                        if let Ok(check) = first_child.downcast::<CheckButton>() {
                            if check.is_active() {
                                let name = check.widget_name();
                                if !name.is_empty() {
                                    selected_names.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        child_opt = next;
    }

    let res = if selected_names.is_empty() {
        audio::enable_combined(None)
    } else {
        audio::enable_combined(Some(&selected_names))
    };

    match res {
        Ok(()) => title.set_subtitle("Combined output enabled."),
        Err(e) => title.set_subtitle(&format!("Failed to enable combined sink: {e}")),
    }
}
