//! Keyboard-shortcut dispatch: pure index helpers, the injected listener JS,
//! and the `handle_shortcut` command. Built up across tasks 1, 3, 4, 6.

use tauri::{AppHandle, State};

use crate::state::AppState;

/// Cycle direction for [`cycle_index`].
pub enum CycleDir {
    Next,
    Prev,
}

/// Wrapping index for cycling apps. `current` is the active app's index; returns
/// the next/previous index, wrapping. `None` if the list is empty.
pub fn cycle_index(current: usize, len: usize, dir: CycleDir) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match dir {
        CycleDir::Next => (current + 1) % len,
        CycleDir::Prev => (current + len - 1) % len,
    })
}

/// 1-based positional jump → 0-based index. `None` for 0 or past the end. No wrap.
pub fn jump_index(n: usize, len: usize) -> Option<usize> {
    if n == 0 || n > len {
        None
    } else {
        Some(n - 1)
    }
}

/// After removing the app at `removed` from a list of `len_before`, which
/// surviving index to activate? Prefers the app that was after (it slides into
/// `removed`'s slot); else the new last; else `None` (was the only app). No wrap.
pub fn neighbor_index(removed: usize, len_before: usize) -> Option<usize> {
    if len_before <= 1 {
        return None;
    }
    if removed + 1 < len_before {
        Some(removed)
    } else {
        Some(removed - 1)
    }
}

/// JS injected into every app webview. Listens for our shortcut bindings in the
/// capture phase (so the hosted app never sees them — the shell wins, matching
/// Slack/Rambox), then forwards the matched action to `handle_shortcut`.
///
/// The key→action table MIRRORS the one in `src/lib/shortcuts.ts`. Both map onto
/// the same closed set of action strings — keep them in sync when editing.
pub fn build_shortcut_listener_js() -> &'static str {
    r#"
(function() {
  if (window.__webapps_shortcut_listener) return;
  window.__webapps_shortcut_listener = true;

  // (ctrl, shift, keyLower) -> action
  var TABLE = {
    "true|false|tab": "cycle-next",
    "true|true|tab": "cycle-prev",
    "true|false|b": "toggle-sidebar",
    "true|false|n": "add-app",
    "true|false|w": "sleep-app",
    "true|true|s": "space-switcher"
  };

  function actionFor(e) {
    var keyLower = (e.key || "").toLowerCase();
    // Ctrl+1..9 (no shift, no alt, no meta)
    if (e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey
        && keyLower.length === 1 && keyLower >= "1" && keyLower <= "9") {
      return "jump-" + keyLower;
    }
    if (!e.ctrlKey || e.metaKey || e.altKey) return null;
    var k = "true|" + (e.shiftKey ? "true" : "false") + "|" + keyLower;
    return TABLE[k] || null;
  }

  document.addEventListener("keydown", function(e) {
    var action = actionFor(e);
    if (!action) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (window.__TAURI_INTERNALS__) {
      try { window.__TAURI_INTERNALS__.invoke("handle_shortcut", { action: action }); }
      catch (err) { /* ignore */ }
    }
  }, true);
})();
"#
}

/// Read a snapshot of the active space: `(space_id, ordered app_ids, active_app_id)`.
fn active_space_apps(state: &AppState) -> Result<(String, Vec<String>, Option<String>), String> {
    let active_space_id = state.active_space_id.lock().map_err(|e| e.to_string())?.clone();
    let active_app_id = state.active_app_id.lock().map_err(|e| e.to_string())?.clone();
    let app_ids = {
        let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let space = spaces
            .iter()
            .find(|s| s.space.id == active_space_id)
            .ok_or_else(|| format!("Space '{active_space_id}' not found"))?;
        space.apps.iter().map(|a| a.id.clone()).collect::<Vec<_>>()
    };
    Ok((active_space_id, app_ids, active_app_id))
}

/// Single dispatch point for every keyboard shortcut. The injected app-webview
/// listener and the Svelte shell listener both `invoke("handle_shortcut", { action })`.
#[tauri::command]
pub fn handle_shortcut(
    app_handle: AppHandle,
    action: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    match action.as_str() {
        "cycle-next" | "cycle-prev" => {
            let dir = if action == "cycle-next" {
                CycleDir::Next
            } else {
                CycleDir::Prev
            };
            let (space_id, app_ids, active) = active_space_apps(&state)?;
            if app_ids.is_empty() {
                return Ok(());
            }
            // Nothing active: next opens the first app, prev opens the last.
            let cur = active
                .as_deref()
                .and_then(|a| app_ids.iter().position(|id| id == a));
            let target_idx = match cur {
                Some(c) => cycle_index(c, app_ids.len(), dir),
                None => Some(if matches!(dir, CycleDir::Next) { 0 } else { app_ids.len() - 1 }),
            };
            if let Some(idx) = target_idx {
                let target = app_ids[idx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        s if s.starts_with("jump-") => {
            let n: usize = s["jump-".len()..]
                .parse()
                .map_err(|_| format!("bad jump action: {s}"))?;
            let (space_id, app_ids, _active) = active_space_apps(&state)?;
            if let Some(idx) = jump_index(n, app_ids.len()) {
                let target = app_ids[idx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        "toggle-sidebar" => {
            crate::commands::webviews::toggle_sidebar_inner(&app_handle, &state)?;
            Ok(())
        }
        "add-app" => {
            let space_id = state
                .active_space_id
                .lock()
                .map_err(|e| e.to_string())?
                .clone();
            crate::commands::dialog::show_dialog(
                app_handle,
                "add-app".to_string(),
                Some(space_id),
                None,
            )?;
            Ok(())
        }
        "sleep-app" => {
            let (space_id, app_ids, active) = active_space_apps(&state)?;
            let active_id = match active {
                Some(a) => a,
                None => return Ok(()),
            };
            let pos = match app_ids.iter().position(|id| *id == active_id) {
                Some(p) => p,
                None => return Ok(()),
            };
            // Reversible sleep, then switch to a neighbor (next else prev else none).
            crate::commands::webviews::sleep_app_inner(&app_handle, &active_id, &state)?;
            if let Some(nidx) = neighbor_index(pos, app_ids.len()) {
                let target = app_ids[nidx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        "space-switcher" => {
            crate::commands::dialog::open_space_switcher(app_handle)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_next_wraps() {
        assert_eq!(cycle_index(0, 3, CycleDir::Next), Some(1));
        assert_eq!(cycle_index(2, 3, CycleDir::Next), Some(0));
    }

    #[test]
    fn cycle_prev_wraps() {
        assert_eq!(cycle_index(1, 3, CycleDir::Prev), Some(0));
        assert_eq!(cycle_index(0, 3, CycleDir::Prev), Some(2));
    }

    #[test]
    fn cycle_empty_returns_none() {
        assert_eq!(cycle_index(0, 0, CycleDir::Next), None);
        assert_eq!(cycle_index(0, 0, CycleDir::Prev), None);
    }

    #[test]
    fn jump_one_based_no_wrap() {
        assert_eq!(jump_index(1, 3), Some(0));
        assert_eq!(jump_index(3, 3), Some(2));
        assert_eq!(jump_index(4, 3), None);
        assert_eq!(jump_index(0, 3), None);
    }

    #[test]
    fn neighbor_prefers_after_then_before() {
        assert_eq!(neighbor_index(1, 3), Some(1)); // middle: after slides in
        assert_eq!(neighbor_index(2, 3), Some(1)); // last: new last
        assert_eq!(neighbor_index(0, 3), Some(0)); // first of many: after slides in
    }

    #[test]
    fn neighbor_only_app_returns_none() {
        assert_eq!(neighbor_index(0, 1), None);
        assert_eq!(neighbor_index(0, 0), None);
    }

    #[test]
    fn shortcut_listener_js_contains_all_actions() {
        let js = build_shortcut_listener_js();
        for needle in [
            "cycle-next",
            "cycle-prev",
            "toggle-sidebar",
            "add-app",
            "sleep-app",
            "space-switcher",
            "handle_shortcut",
        ] {
            assert!(js.contains(needle), "shortcut listener JS missing {needle}");
        }
        // Capture phase + shell-wins behavior.
        assert!(js.contains("addEventListener(\"keydown\""));
        assert!(js.contains(", true)"), "listener must bind in the capture phase");
        assert!(js.contains("__webapps_shortcut_listener"), "listener must be idempotent");
        assert!(js.contains("e.preventDefault()"));
        assert!(js.contains("e.stopImmediatePropagation()"));
    }
}
