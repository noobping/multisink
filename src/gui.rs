use crate::audio;
use crate::audio::Sink;
use gtk4::prelude::*;
use gtk4::{
    gio, Application, ApplicationWindow, Builder, Box as GtkBox, Button, CheckButton, Label,
    ListBox,
};

const APP_ID: &str = "dev.nick.multisink";
const UI_SRC: &str = include_str!("../data/multisink.ui");

pub fn run_gui() -> anyhow::Result<()> {
    // If you want to *force* Wayland:
    // std::env::set_var("GDK_BACKEND", "wayland");

    let app = Application::builder()
        .application_id(APP_ID)
        // We handle command line ourselves so GLib doesn't whine about "opening files"
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

    let status_label: Label = builder
        .object("status_label")
        .expect("Failed to get status_label");
    let list_box: ListBox = builder
        .object("sink_list")
        .expect("Failed to get sink_list");

    let refresh_button: Button = builder
        .object("refresh_button")
        .expect("Failed to get refresh_button");
    let enable_button: Button = builder
        .object("enable_button")
        .expect("Failed to get enable_button");
    let disable_button: Button = builder
        .object("disable_button")
        .expect("Failed to get disable_button");

    // Connect buttons
    let list_box_clone = list_box.clone();
    let status_clone = status_label.clone();
    refresh_button.connect_clicked(move |_| {
        refresh_sinks(&list_box_clone, &status_clone);
    });

    let list_box_clone = list_box.clone();
    let status_clone = status_label.clone();
    enable_button.connect_clicked(move |_| {
        enable_from_selection(&list_box_clone, &status_clone);
    });

    let status_clone = status_label.clone();
    disable_button.connect_clicked(move |_| {
        match audio::disable_combined() {
            Ok(()) => status_clone.set_label("Disabled combined sink."),
            Err(e) => status_clone.set_label(&format!("Failed to disable combined sink: {e}")),
        }
    });

    // Initial populate
    refresh_sinks(&list_box, &status_label);

    window.show();
}

fn clear_list_box(list_box: &ListBox) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
}

fn refresh_sinks(list_box: &ListBox, status_label: &Label) {
    clear_list_box(list_box);

    match audio::check_backend() {
        Err(e) => {
            status_label.set_label(&format!("Audio backend not available: {e}"));
            return;
        }
        Ok(()) => {}
    }

    let sinks: Vec<Sink> = match audio::list_sinks() {
        Ok(s) => s,
        Err(e) => {
            status_label.set_label(&format!("Error listing sinks: {e}"));
            return;
        }
    };

    if sinks.is_empty() {
        status_label.set_label("No sinks found.");
        return;
    }

    let combined_present = sinks.iter().any(|s| s.is_combined);

    for s in sinks {
        let row_box = GtkBox::new(gtk4::Orientation::Horizontal, 6);

        let check = CheckButton::new();
        // Store sink name in widget_name to retrieve later
        check.set_widget_name(&s.name);
        check.set_active(!s.is_combined);

        let label_text = if s.is_combined {
            format!("{} (#{} ) [combined]", s.name, s.index)
        } else {
            format!("{} (#{} )", s.name, s.index)
        };
        let label = Label::new(Some(&label_text));
        label.set_xalign(0.0);

        row_box.append(&check);
        row_box.append(&label);

        list_box.append(&row_box);
    }

    if combined_present {
        status_label.set_label("Backend OK. Combined sink is present.");
    } else {
        status_label.set_label("Backend OK. No combined sink yet.");
    }
}

fn enable_from_selection(list_box: &ListBox, status_label: &Label) {
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
        Ok(()) => status_label.set_label("Enabled combined sink."),
        Err(e) => status_label.set_label(&format!("Failed to enable combined sink: {e}")),
    }
}
