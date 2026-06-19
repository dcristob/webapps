//! DMA-BUF renderer self-heal for WebKitGTK on Linux.
//!
//! WebKitGTK's DMA-BUF GPU renderer crashes on some Mesa/GPU/Wayland combos
//! (symptom: "Failed to create GBM buffer" / "Protocol error dispatching to
//! Wayland display" at startup). The crash kills the process, so it can't be
//! caught in-process. Instead we probe: default DMA-BUF ON, write a `probing`
//! marker at startup, and an `ok` marker once the app has run long enough that
//! a startup GPU crash would already have happened. If a launch leaves a stale
//! `probing` marker (it crashed), the NEXT launch disables DMA-BUF and
//! persists that choice — so working systems keep GPU accel and broken ones
//! self-heal after a single first-launch crash.

use crate::config::storage;

const MARKER_FILENAME: &str = "dmabuf-mode";

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum DmabufMode {
    /// DMA-BUF renderer enabled (GPU acceleration on).
    Enabled,
    /// DMA-BUF renderer disabled (CPU / shared-memory fallback).
    Disabled,
}

/// Decide the renderer mode for this launch.
///
/// - `manual_disable`: the `WEBAPPS_DISABLE_DMABUF=1` override was set.
/// - `marker`: the current marker file contents (`None` if absent). A stale
///   `probing` means the previous launch crashed before confirming.
fn decide_mode(manual_disable: bool, marker: Option<&str>) -> DmabufMode {
    if manual_disable {
        return DmabufMode::Disabled;
    }
    match marker {
        // "disabled": previously determined unsafe. "probing": the previous
        // launch crashed before it could confirm — treat as unsafe too.
        Some("disabled") | Some("probing") => DmabufMode::Disabled,
        // Absent, "ok", or any unrecognized value: keep DMA-BUF on.
        _ => DmabufMode::Enabled,
    }
}

/// Marker value to write at startup, or `None` to leave the file unchanged.
fn startup_write(mode: DmabufMode, manual_disable: bool, prev_marker: Option<&str>) -> Option<&'static str> {
    match mode {
        DmabufMode::Enabled => Some("probing"),
        DmabufMode::Disabled if manual_disable => None, // override is transient
        DmabufMode::Disabled if prev_marker == Some("probing") => Some("disabled"), // persist crash detection
        DmabufMode::Disabled => None, // already "disabled" (or unknown) — leave as-is
    }
}

fn marker_path() -> Result<std::path::PathBuf, storage::ConfigError> {
    storage::config_dir().map(|d| d.join(MARKER_FILENAME))
}

fn read_marker() -> Option<String> {
    marker_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
}

/// True when the user set `WEBAPPS_DISABLE_DMABUF=1` (one-shot override;
/// does not persist to the marker file).
fn manual_disable_override() -> bool {
    matches!(
        std::env::var("WEBAPPS_DISABLE_DMABUF").ok().as_deref(),
        Some("1")
    )
}

/// Configure the renderer for this launch. Call BEFORE any GTK/WebKit init
/// (i.e. before the Tauri builder runs). Sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`
/// when disabled and updates the marker to track probe state.
#[cfg(target_os = "linux")]
pub fn configure_for_launch() {
    let manual = manual_disable_override();
    let prev = read_marker();
    let mode = decide_mode(manual, prev.as_deref());

    if mode == DmabufMode::Disabled {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    if let Some(value) = startup_write(mode, manual, prev.as_deref()) {
        if let Ok(path) = marker_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, value);
        }
    }
}

/// Confirm the app started without crashing, so future launches know DMA-BUF
/// is safe. Call after the app has been live long enough that a startup GPU
/// crash would already have killed the process.
#[cfg(target_os = "linux")]
pub fn confirm_started_ok() {
    if read_marker().as_deref() == Some("probing") {
        if let Ok(path) = marker_path() {
            let _ = std::fs::write(&path, "ok");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_enables_and_probes() {
        assert_eq!(decide_mode(false, None), DmabufMode::Enabled);
        assert_eq!(startup_write(DmabufMode::Enabled, false, None), Some("probing"));
    }

    #[test]
    fn confirmed_ok_stays_enabled_and_reprobes() {
        assert_eq!(decide_mode(false, Some("ok")), DmabufMode::Enabled);
        assert_eq!(
            startup_write(DmabufMode::Enabled, false, Some("ok")),
            Some("probing")
        );
    }

    #[test]
    fn stale_probing_means_crash_disables_and_persists() {
        assert_eq!(decide_mode(false, Some("probing")), DmabufMode::Disabled);
        assert_eq!(
            startup_write(DmabufMode::Disabled, false, Some("probing")),
            Some("disabled")
        );
    }

    #[test]
    fn already_disabled_stays_disabled_without_rewrite() {
        assert_eq!(decide_mode(false, Some("disabled")), DmabufMode::Disabled);
        assert_eq!(
            startup_write(DmabufMode::Disabled, false, Some("disabled")),
            None
        );
    }

    #[test]
    fn unknown_marker_value_is_treated_as_first_launch() {
        // Garbage in the file shouldn't lock the user out of GPU accel.
        assert_eq!(decide_mode(false, Some("???")), DmabufMode::Enabled);
        assert_eq!(
            startup_write(DmabufMode::Enabled, false, Some("???")),
            Some("probing")
        );
    }

    #[test]
    fn manual_override_disables_without_persisting() {
        assert_eq!(decide_mode(true, Some("ok")), DmabufMode::Disabled);
        assert_eq!(startup_write(DmabufMode::Disabled, true, Some("ok")), None);
    }

    #[test]
    fn manual_override_wins_over_stale_probing_and_does_not_persist() {
        assert_eq!(decide_mode(true, Some("probing")), DmabufMode::Disabled);
        // The override is transient: don't write "disabled" — the next
        // un-overridden launch will still see "probing" and self-heal.
        assert_eq!(
            startup_write(DmabufMode::Disabled, true, Some("probing")),
            None
        );
    }
}
