//! WebKitGTK renderer self-heal for Linux — an escalation ladder.
//!
//! Some GPU/driver/compositor combos make WebKitGTK unstable: on a bad day the
//! DMA-BUF GPU renderer crashes the web process at startup ("Failed to create
//! GBM buffer"), and on a worse day (seen with the NVIDIA proprietary driver
//! under Wayland) accelerated compositing hard-locks the whole machine. Neither
//! can be caught in-process — the crash/freeze kills us first.
//!
//! So we probe and step down only as far as needed, per machine:
//!
//!   Full            → GPU compositing on, DMA-BUF on           (the default)
//!   NoDmabuf        → DMA-BUF renderer off                     (WEBKIT_DISABLE_DMABUF_RENDERER)
//!   NoCompositing   → + accelerated compositing off, prefer X11 (WEBKIT_DISABLE_COMPOSITING_MODE, GDK_BACKEND=x11)
//!
//! Each launch writes a `probing:<tier>` marker, then—after the app has run
//! long enough that a startup crash would already have happened—rewrites it to
//! `ok:<tier>`. If a launch leaves a stale `probing:<tier>` (it crashed or froze
//! the box, which got force-reset), the NEXT launch escalates to the next tier
//! down and persists that. A machine that never crashes stays at `Full` forever,
//! so we never remove GPU acceleration from hardware that doesn't need it.

use crate::config::storage;

const MARKER_FILENAME: &str = "dmabuf-mode";

/// How much of the GPU path WebKitGTK is allowed to use this launch.
/// Ordered least-conservative (`Full`) to most (`NoCompositing`).
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum RenderTier {
    Full,
    NoDmabuf,
    NoCompositing,
}

impl RenderTier {
    fn as_str(self) -> &'static str {
        match self {
            RenderTier::Full => "full",
            RenderTier::NoDmabuf => "nodmabuf",
            RenderTier::NoCompositing => "nocompositing",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "full" => Some(RenderTier::Full),
            "nodmabuf" => Some(RenderTier::NoDmabuf),
            "nocompositing" => Some(RenderTier::NoCompositing),
            _ => None,
        }
    }

    /// The next tier down (more conservative). `NoCompositing` is the floor.
    fn escalate(self) -> Self {
        match self {
            RenderTier::Full => RenderTier::NoDmabuf,
            RenderTier::NoDmabuf => RenderTier::NoCompositing,
            RenderTier::NoCompositing => RenderTier::NoCompositing,
        }
    }
}

/// Parsed marker: `(was_probing, tier)`. `was_probing == true` means the
/// previous launch was still probing that tier when it died — i.e. it crashed.
fn parse_marker(marker: Option<&str>) -> Option<(bool, RenderTier)> {
    let raw = marker?.trim();
    // Back-compat with the original two-state marker.
    match raw {
        "probing" => return Some((true, RenderTier::Full)),
        "ok" => return Some((false, RenderTier::Full)),
        "disabled" => return Some((false, RenderTier::NoDmabuf)),
        _ => {}
    }
    let (state, tier) = raw.split_once(':')?;
    let tier = RenderTier::from_str(tier)?;
    match state {
        "probing" => Some((true, tier)),
        "ok" => Some((false, tier)),
        _ => None,
    }
}

/// Decide the tier for this launch. An explicit `override_tier` (from env) wins;
/// otherwise a stale `probing` marker escalates, a confirmed `ok` marker holds,
/// and anything else (absent/garbage) starts at `Full`.
fn decide_tier(override_tier: Option<RenderTier>, marker: Option<&str>) -> RenderTier {
    if let Some(t) = override_tier {
        return t;
    }
    match parse_marker(marker) {
        Some((true, tier)) => tier.escalate(), // crashed while probing → step down
        Some((false, tier)) => tier,           // confirmed working → hold
        None => RenderTier::Full,
    }
}

/// Marker to write at startup, or `None` to leave the file untouched (used for
/// the transient env override so it never poisons the persisted state).
fn startup_write(tier: RenderTier, is_override: bool) -> Option<String> {
    if is_override {
        None
    } else {
        Some(format!("probing:{}", tier.as_str()))
    }
}

/// Read an explicit tier override from the environment.
/// - `WEBAPPS_RENDER_TIER=full|nodmabuf|nocompositing` — full control.
/// - `WEBAPPS_DISABLE_DMABUF=1` — legacy shorthand for `nodmabuf`.
fn override_tier() -> Option<RenderTier> {
    if let Ok(v) = std::env::var("WEBAPPS_RENDER_TIER") {
        if let Some(t) = RenderTier::from_str(v.trim()) {
            return Some(t);
        }
    }
    if std::env::var("WEBAPPS_DISABLE_DMABUF").ok().as_deref() == Some("1") {
        return Some(RenderTier::NoDmabuf);
    }
    None
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

fn write_marker(value: &str) {
    if let Ok(path) = marker_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, value);
    }
}

/// Apply the environment for a tier. Must run BEFORE any GTK/WebKit init.
#[cfg(target_os = "linux")]
fn apply_tier_env(tier: RenderTier) {
    if tier == RenderTier::Full {
        return;
    }
    // NoDmabuf and below: turn off the DMA-BUF GPU renderer.
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    if tier == RenderTier::NoCompositing {
        // Turn off accelerated compositing entirely (software paint).
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        // Prefer XWayland: the NVIDIA proprietary driver is far more stable for
        // GTK/WebKit compositing on X11 than on native Wayland. Only nudge it
        // when we're actually on Wayland and the user hasn't already chosen.
        if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
            && std::env::var_os("GDK_BACKEND").is_none()
        {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }
}

/// Configure the renderer for this launch. Call BEFORE the Tauri builder runs.
#[cfg(target_os = "linux")]
pub fn configure_for_launch() {
    let ovr = override_tier();
    let prev = read_marker();
    let tier = decide_tier(ovr, prev.as_deref());

    apply_tier_env(tier);

    if let Some(value) = startup_write(tier, ovr.is_some()) {
        write_marker(&value);
    }
}

/// Confirm the app started without crashing/freezing at the probed tier, so
/// future launches keep it instead of escalating. Call after a short delay.
#[cfg(target_os = "linux")]
pub fn confirm_started_ok() {
    if let Some((true, tier)) = parse_marker(read_marker().as_deref()) {
        write_marker(&format!("ok:{}", tier.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_is_full_and_probes() {
        assert_eq!(decide_tier(None, None), RenderTier::Full);
        assert_eq!(
            startup_write(RenderTier::Full, false).as_deref(),
            Some("probing:full")
        );
    }

    #[test]
    fn confirmed_full_holds_and_reprobes() {
        assert_eq!(decide_tier(None, Some("ok:full")), RenderTier::Full);
        assert_eq!(
            startup_write(RenderTier::Full, false).as_deref(),
            Some("probing:full")
        );
    }

    #[test]
    fn crash_at_full_escalates_to_nodmabuf() {
        assert_eq!(decide_tier(None, Some("probing:full")), RenderTier::NoDmabuf);
    }

    #[test]
    fn crash_at_nodmabuf_escalates_to_nocompositing() {
        assert_eq!(
            decide_tier(None, Some("probing:nodmabuf")),
            RenderTier::NoCompositing
        );
    }

    #[test]
    fn nocompositing_is_the_floor() {
        assert_eq!(
            decide_tier(None, Some("probing:nocompositing")),
            RenderTier::NoCompositing
        );
    }

    #[test]
    fn confirmed_tier_holds_without_dropping_back() {
        assert_eq!(
            decide_tier(None, Some("ok:nocompositing")),
            RenderTier::NoCompositing
        );
    }

    #[test]
    fn unknown_marker_falls_back_to_full() {
        assert_eq!(decide_tier(None, Some("???")), RenderTier::Full);
        assert_eq!(decide_tier(None, Some("probing:garbage")), RenderTier::Full);
    }

    #[test]
    fn override_wins_and_does_not_persist() {
        assert_eq!(
            decide_tier(Some(RenderTier::NoCompositing), Some("ok:full")),
            RenderTier::NoCompositing
        );
        assert_eq!(startup_write(RenderTier::NoCompositing, true), None);
    }

    // --- back-compat with the original two-state marker ---

    #[test]
    fn legacy_probing_marker_escalates() {
        assert_eq!(decide_tier(None, Some("probing")), RenderTier::NoDmabuf);
    }

    #[test]
    fn legacy_ok_marker_is_full() {
        assert_eq!(decide_tier(None, Some("ok")), RenderTier::Full);
    }

    #[test]
    fn legacy_disabled_marker_maps_to_nodmabuf() {
        assert_eq!(decide_tier(None, Some("disabled")), RenderTier::NoDmabuf);
    }
}
