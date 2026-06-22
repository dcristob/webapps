import { invoke } from "@tauri-apps/api/core";

// Shell-side binding table. MIRRORS the injected app-webview table in
// src-tauri/src/commands/shortcuts.rs (build_shortcut_listener_js). Both map
// onto the same closed set of action strings — keep them in sync.
//
// This listener covers the case where a shell webview (sidebar/topbar) has
// keyboard focus. Shortcuts typed inside a hosted app are caught by the
// injected JS instead.

function actionFor(e: KeyboardEvent): string | null {
  const keyLower = e.key.toLowerCase();

  // Ctrl+1..9 (no shift/alt/meta)
  if (
    e.ctrlKey &&
    !e.metaKey &&
    !e.altKey &&
    !e.shiftKey &&
    keyLower.length === 1 &&
    keyLower >= "1" &&
    keyLower <= "9"
  ) {
    return "jump-" + keyLower;
  }
  if (!e.ctrlKey || e.metaKey || e.altKey) return null;

  const k = `true|${e.shiftKey ? "true" : "false"}|${keyLower}`;
  switch (k) {
    case "true|false|tab":
      return "cycle-next";
    case "true|true|tab":
      return "cycle-prev";
    case "true|false|b":
      return "toggle-sidebar";
    case "true|false|n":
      return "add-app";
    case "true|false|w":
      return "sleep-app";
    case "true|true|s":
      return "space-switcher";
    default:
      return null;
  }
}

/** Attach the shell keydown listener (call in sidebar/topbar webviews).
 *  Returns a cleanup function. */
export function installShellShortcuts(): () => void {
  const handler = (e: KeyboardEvent) => {
    const action = actionFor(e);
    if (!action) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    void invoke("handle_shortcut", { action });
  };
  window.addEventListener("keydown", handler, true);
  return () => window.removeEventListener("keydown", handler, true);
}
