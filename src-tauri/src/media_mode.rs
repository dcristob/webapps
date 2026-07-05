//! Media-capture self-heal for WebKitGTK on Linux — an escalation ladder that
//! keeps the microphone/camera working where it can.
//!
//! On some PipeWire/GStreamer/WebKit combos the web-content process SIGSEGVs
//! while enumerating capture devices: the crash lands in the GStreamer PipeWire
//! device provider (`gst_device_provider_start` →
//! `libpipewire-module-protocol-native`, jumping through a corrupt pointer).
//! Any page that touches `navigator.mediaDevices` then takes the webview down.
//!
//! GStreamer also ships Pulse/ALSA/V4L2 device providers that enumerate the
//! same hardware without crashing. So rather than killing capture outright, we
//! step down only as far as needed, per machine:
//!
//!   Native      → stock GStreamer, PipeWire provider active   (the default)
//!   NoPipewire  → exclude ONLY the PipeWire GStreamer plugin;  capture still
//!                 works via Pulse/ALSA/V4L2, and the crash can't happen
//!   Off         → last resort: disable `media-stream`/`webrtc` entirely
//!
//! Machines that never crash stay `Native` and keep the native provider.
//!
//! `NoPipewire` works by pointing the app's processes at a private GStreamer
//! plugin directory — a symlink farm of every system plugin EXCEPT
//! `libgstpipewire.so` — via `GST_PLUGIN_SYSTEM_PATH_1_0`, plus a private
//! `GST_REGISTRY` so the user's normal cache is untouched. WebKit's web
//! processes inherit these env vars and simply never load the broken provider.

use crate::config::storage;

const MARKER_FILENAME: &str = "media-mode";
const EXCLUDED_PLUGIN: &str = "libgstpipewire.so";

/// How much of the media path is enabled this launch. Ordered least-conservative
/// (`Native`) to most (`Off`).
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum MediaTier {
    Native,
    NoPipewire,
    Off,
}

impl MediaTier {
    fn as_str(self) -> &'static str {
        match self {
            MediaTier::Native => "native",
            MediaTier::NoPipewire => "no-pipewire",
            MediaTier::Off => "off",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "native" => Some(MediaTier::Native),
            "no-pipewire" => Some(MediaTier::NoPipewire),
            "off" => Some(MediaTier::Off),
            // Back-compat: the original self-heal wrote "disabled". Prefer the
            // mic-preserving tier now — it self-escalates to Off if it still crashes.
            "disabled" => Some(MediaTier::NoPipewire),
            _ => None,
        }
    }

    /// The next tier down (more conservative). `Off` is the floor.
    fn escalate(self) -> Self {
        match self {
            MediaTier::Native => MediaTier::NoPipewire,
            MediaTier::NoPipewire => MediaTier::Off,
            MediaTier::Off => MediaTier::Off,
        }
    }

    /// Whether WebKit `media-stream`/`webrtc` should stay enabled at this tier.
    pub fn media_stream_enabled(self) -> bool {
        self != MediaTier::Off
    }
}

/// Decide the tier for this launch. An explicit `override_tier` (env) wins;
/// otherwise the persisted marker holds.
fn decide_tier(override_tier: Option<MediaTier>, marker: Option<&str>) -> MediaTier {
    if let Some(t) = override_tier {
        return t;
    }
    marker
        .and_then(MediaTier::from_str)
        .unwrap_or(MediaTier::Native)
}

/// `WEBAPPS_MEDIA=native|no-pipewire|off` forces a tier (bypassing the marker).
fn override_tier() -> Option<MediaTier> {
    std::env::var("WEBAPPS_MEDIA")
        .ok()
        .and_then(|v| MediaTier::from_str(&v))
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

fn write_marker(tier: MediaTier) {
    if let Ok(path) = marker_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, tier.as_str());
    }
}

/// The tier decided for this launch (env override or persisted marker).
pub fn current_tier() -> MediaTier {
    decide_tier(override_tier(), read_marker().as_deref())
}

/// Configure the media env for this launch. Call BEFORE any GTK/WebKit init so
/// WebKit's web processes inherit it. For `NoPipewire` this builds the filtered
/// GStreamer plugin path; other tiers touch nothing here.
#[cfg(target_os = "linux")]
pub fn configure_for_launch() {
    if current_tier() == MediaTier::NoPipewire {
        apply_no_pipewire_env();
    }
}

/// React to a web-process crash: escalate one tier and persist it. Returns the
/// new tier so the caller can apply it live (env for the reloaded process,
/// per-webview settings) — or `None` if we were already at the floor / an env
/// override pins the tier (nothing to persist).
#[cfg(target_os = "linux")]
pub fn escalate_on_crash() -> Option<MediaTier> {
    if override_tier().is_some() {
        return None; // user pinned the tier; don't fight them
    }
    let current = current_tier();
    let next = current.escalate();
    if next == current {
        return None; // already at Off
    }
    write_marker(next);
    if next == MediaTier::NoPipewire {
        apply_no_pipewire_env();
    }
    Some(next)
}

/// Locate the system GStreamer 1.0 plugin directory.
#[cfg(target_os = "linux")]
fn system_gst_plugin_dir() -> Option<std::path::PathBuf> {
    // Honour an already-set path first, then the usual distro locations.
    if let Some(p) = std::env::var_os("GST_PLUGIN_SYSTEM_PATH_1_0") {
        let path = std::path::PathBuf::from(p);
        if path.is_dir() {
            return Some(path);
        }
    }
    ["/usr/lib/gstreamer-1.0", "/usr/lib64/gstreamer-1.0", "/usr/lib/x86_64-linux-gnu/gstreamer-1.0"]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.join(EXCLUDED_PLUGIN).exists())
}

/// Build a symlink farm of every system GStreamer plugin except the PipeWire
/// one, and point this process (and its children) at it via env vars.
///
/// The farm is cached and only rebuilt when the system plugin set changes —
/// rebuilding every launch would give the symlinks fresh mtimes and force
/// GStreamer to re-scan its whole registry, slowing startup noticeably.
#[cfg(target_os = "linux")]
fn apply_no_pipewire_env() {
    let Some(src) = system_gst_plugin_dir() else {
        return; // no system dir / no pipewire plugin to exclude — nothing to do
    };
    let Some(cache) = dirs::cache_dir() else { return };
    let base = cache.join("webapps").join("gst-nopipewire");
    let farm = base.join("plugins");
    let stamp_path = base.join("plugins.stamp");

    // Signature of the source dir: "<count>:<newest-mtime-secs>". If it matches
    // the cached stamp and the farm exists, reuse it untouched.
    let signature = plugin_dir_signature(&src);
    let cached = std::fs::read_to_string(&stamp_path).ok();
    let fresh = farm.is_dir() && signature.as_deref() == cached.as_deref().map(str::trim);

    if !fresh {
        let _ = std::fs::remove_dir_all(&farm);
        if std::fs::create_dir_all(&farm).is_ok() {
            if let Ok(entries) = std::fs::read_dir(&src) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name == EXCLUDED_PLUGIN {
                        continue;
                    }
                    let is_so = std::path::Path::new(&name)
                        .extension()
                        .and_then(|e| e.to_str())
                        == Some("so");
                    if is_so {
                        let _ = std::os::unix::fs::symlink(entry.path(), farm.join(&name));
                    }
                }
            }
            if let Some(sig) = &signature {
                let _ = std::fs::write(&stamp_path, sig);
            }
        }
    }

    std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &farm);
    // Keep GStreamer's registry scan for this filtered path out of the user's
    // default cache so we don't cause churn for their other GStreamer apps.
    std::env::set_var("GST_REGISTRY", base.join("registry.bin"));
}

/// `"<count>:<newest-mtime-secs>"` over the `.so` files in `dir`, or `None`.
#[cfg(target_os = "linux")]
fn plugin_dir_signature(dir: &std::path::Path) -> Option<String> {
    use std::time::UNIX_EPOCH;
    let entries = std::fs::read_dir(dir).ok()?;
    let mut count: u64 = 0;
    let mut newest: u64 = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if std::path::Path::new(&name).extension().and_then(|e| e.to_str()) != Some("so") {
            continue;
        }
        count += 1;
        if let Ok(secs) = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        {
            newest = newest.max(secs);
        }
    }
    Some(format!("{}:{}", count, newest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_native() {
        assert_eq!(decide_tier(None, None), MediaTier::Native);
        assert_eq!(decide_tier(None, Some("garbage")), MediaTier::Native);
    }

    #[test]
    fn marker_selects_tier() {
        assert_eq!(decide_tier(None, Some("no-pipewire")), MediaTier::NoPipewire);
        assert_eq!(decide_tier(None, Some("off")), MediaTier::Off);
        assert_eq!(decide_tier(None, Some(" native ")), MediaTier::Native);
    }

    #[test]
    fn legacy_disabled_marker_prefers_mic_preserving_tier() {
        assert_eq!(decide_tier(None, Some("disabled")), MediaTier::NoPipewire);
    }

    #[test]
    fn env_override_wins_over_marker() {
        assert_eq!(
            decide_tier(Some(MediaTier::Off), Some("no-pipewire")),
            MediaTier::Off
        );
    }

    #[test]
    fn escalation_ladder() {
        assert_eq!(MediaTier::Native.escalate(), MediaTier::NoPipewire);
        assert_eq!(MediaTier::NoPipewire.escalate(), MediaTier::Off);
        assert_eq!(MediaTier::Off.escalate(), MediaTier::Off);
    }

    #[test]
    fn only_off_disables_media_stream() {
        assert!(MediaTier::Native.media_stream_enabled());
        assert!(MediaTier::NoPipewire.media_stream_enabled());
        assert!(!MediaTier::Off.media_stream_enabled());
    }
}
