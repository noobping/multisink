use std::process::Command;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

pub const COMBINED_SINK_NAME: &str = "multisink_combined";

#[derive(Debug, Clone)]
pub struct Sink {
    pub index: u32,
    pub name: String,
    pub pretty_name: String,
    pub is_combined: bool,
}

#[derive(Debug, serde::Deserialize)]
struct PactlSink {
    index: u32,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("pactl / PulseAudio/PipeWire backend not available")]
    BackendUnavailable,

    #[error("command failed: {0}")]
    CommandFailed(String),

    #[error("UTF-8 error")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("not enough sinks to create a combined sink")]
    NotEnoughSinks,

    #[error("combined sink not found")]
    CombinedNotFound,
}

fn module_id_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("multisink_module_id")
    } else {
        std::env::temp_dir().join("multisink_module_id")
    }
}

fn set_volume_max_for_sinks(names: &[String]) -> Result<(), AudioError> {
    for name in names {
        // We don't treat failures as fatal; just try best-effort
        let _ = Command::new("pactl")
            .args(["set-sink-volume", name, "100%"])
            .status();
    }
    Ok(())
}

/// Check if we can talk to PulseAudio / PipeWire via pactl.
pub fn check_backend() -> Result<(), AudioError> {
    let output = Command::new("pactl").arg("info").output();

    match output {
        Ok(o) if o.status.success() => Ok(()),
        _ => Err(AudioError::BackendUnavailable),
    }
}

/// List all sinks using `pactl list sinks short`.
pub fn list_sinks() -> Result<Vec<Sink>, AudioError> {
    check_backend()?;

    let output = Command::new("pactl")
        .args(["--format=json", "list", "sinks"])
        .output()?;

    if !output.status.success() {
        return Err(AudioError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }

    let sinks_raw: Vec<PactlSink> = serde_json::from_slice(&output.stdout)
        .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

    let sinks = sinks_raw
        .into_iter()
        .map(|s| {
            let is_combined = s.name == COMBINED_SINK_NAME;
            Sink {
                index: s.index,
                name: s.name.clone(),
                pretty_name: s
                    .description
                    .unwrap_or_else(|| s.name.clone()),
                is_combined,
            }
        })
        .collect();

    Ok(sinks)
}

/// Check if our combined sink exists.
pub fn combined_sink_exists() -> Result<bool, AudioError> {
    Ok(list_sinks()?
        .iter()
        .any(|s| s.name == COMBINED_SINK_NAME))
}

/// Enable combined sink using selected sinks or all (except combined).
///
/// If `selected` is `None`, all physical sinks are used.
/// If `selected` is `Some`, only the provided sink names are used.
pub fn enable_combined(selected: Option<&[String]>) -> Result<(), AudioError> {
    check_backend()?;

    let sinks = list_sinks()?;

    // Decide which sink names to combine (only non-combined, as before)
    let sink_names: Vec<String> = if let Some(sel) = selected {
        sinks
            .into_iter()
            .filter(|s| sel.contains(&s.name) && !s.is_combined)
            .map(|s| s.name)
            .collect()
    } else {
        sinks
            .into_iter()
            .filter(|s| !s.is_combined)
            .map(|s| s.name)
            .collect()
    };

    if sink_names.len() < 2 {
        return Err(AudioError::NotEnoughSinks);
    }

    // Set all selected sinks to 100%
    let _ = set_volume_max_for_sinks(&sink_names);

    let slaves = sink_names.join(",");

    // Create combined sink
    let output = Command::new("pactl")
        .arg("load-module")
        .arg("module-combine-sink")
        .arg(format!("sink_name={}", COMBINED_SINK_NAME))
        .arg(format!("slaves={}", slaves))
        .output()?;

    if !output.status.success() {
        return Err(AudioError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }

    // pactl load-module prints the numeric module id on stdout
    let stdout = String::from_utf8(output.stdout)?;
    let module_id = stdout.trim();
    if !module_id.is_empty() {
        let _ = fs::write(module_id_path(), module_id);
    }

    // Set the combined sink itself to 100%
    let _ = Command::new("pactl")
        .args(["set-sink-volume", COMBINED_SINK_NAME, "100%"])
        .status();

    // Set combined sink as default
    let _ = Command::new("pactl")
        .args(["set-default-sink", COMBINED_SINK_NAME])
        .status();

    // Move existing sink inputs to the new combined sink
    move_existing_streams_to_combined()?;

    Ok(())
}

/// Move current playback streams to the combined sink.
fn move_existing_streams_to_combined() -> Result<(), AudioError> {
    let output = Command::new("pactl")
        .args(["list", "sink-inputs", "short"])
        .output()?;

    if !output.status.success() {
        // Not fatal, just bail quietly
        return Ok(());
    }

    let stdout = String::from_utf8(output.stdout)?;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Format: id \t sink \t ...
        let mut parts = line.split_whitespace();
        let id_str = parts.next().unwrap_or("");
        if id_str.is_empty() {
            continue;
        }

        let _ = Command::new("pactl")
            .args([
                "move-sink-input",
                id_str,
                COMBINED_SINK_NAME,
            ])
            .status();
    }

    Ok(())
}

/// Disable our combined sink by unloading its module.
pub fn disable_combined() -> Result<(), AudioError> {
    check_backend()?;

    // First try unloading by stored module id, if we have one
    if let Ok(id_str) = fs::read_to_string(module_id_path()) {
        let id = id_str.trim();
        if !id.is_empty() {
            let unload = Command::new("pactl")
                .args(["unload-module", id])
                .output()?;

            if unload.status.success() {
                let _ = fs::remove_file(module_id_path());
                return Ok(());
            }
        }
    }

    // Fallback: find the module by scanning pactl list modules
    let output = Command::new("pactl")
        .args(["list", "modules"])
        .output()?;

    if !output.status.success() {
        return Err(AudioError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut current_id: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut current_args: Option<String> = None;
    let mut target_module_id: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();

        if line.starts_with("Module #") {
            // Check previous
            if let (Some(id), Some(name), Some(args)) =
                (&current_id, &current_name, &current_args)
            {
                if name == "module-combine-sink"
                    && args.contains(&format!("sink_name={}", COMBINED_SINK_NAME))
                {
                    target_module_id = Some(id.clone());
                    break;
                }
            }

            current_id = Some(line.trim_start_matches("Module #").trim().to_string());
            current_name = None;
            current_args = None;
        } else if line.starts_with("Name:") {
            current_name = Some(line.trim_start_matches("Name:").trim().to_string());
        } else if line.starts_with("Argument:") {
            current_args = Some(line.trim_start_matches("Argument:").trim().to_string());
        }
    }

    // Last module block
    if target_module_id.is_none() {
        if let (Some(id), Some(name), Some(args)) =
            (&current_id, &current_name, &current_args)
        {
            if name == "module-combine-sink"
                && args.contains(&format!("sink_name={}", COMBINED_SINK_NAME))
            {
                target_module_id = Some(id.clone());
            }
        }
    }

    let module_id = target_module_id.ok_or(AudioError::CombinedNotFound)?;

    let unload = Command::new("pactl")
        .args(["unload-module", &module_id])
        .output()?;

    if !unload.status.success() {
        return Err(AudioError::CommandFailed(
            String::from_utf8_lossy(&unload.stderr).into(),
        ));
    }

    let _ = fs::remove_file(module_id_path());

    Ok(())
}
