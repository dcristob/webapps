# Install Script — Design

**Date:** 2026-06-19
**Status:** Approved

## Goal

Replace the manual install steps documented in CLAUDE.md ("Local Installation &
Releases") with a single script that builds WebApps, installs the binary, and
wires up desktop integration (icon + `.desktop` entry). Provide a matching
uninstall path.

## Deliverable

`scripts/install.sh` — a single bash script.

## Modes

| Invocation                         | Behavior                                              |
| ---------------------------------- | ---------------------------------------------------- |
| `./scripts/install.sh`             | Build + install (default)                            |
| `./scripts/install.sh --skip-build`| Install an already-built binary (skip the Tauri build) |
| `./scripts/install.sh --uninstall` | Remove binary, icons, and desktop entry              |
| `./scripts/install.sh --help`      | Usage                                                |

## Install flow

1. **Preflight** — resolve repo root from the script's own path; `cd` there.
   Confirm `npm` and `cargo` are on `PATH` (skip the `cargo` check when
   `--skip-build`). Confirm the built binary exists when `--skip-build`.
2. **Running-instance check** — `pgrep -x webapps`. If a process is found, warn
   that two instances sharing `~/.config/webapps/webview-data/` corrupt each
   other's WebKit state, then offer to kill it (`y/N`). Abort if declined.
3. **Build** — `npm run tauri -- build --no-bundle` (skipped with `--skip-build`).
4. **Verify embed** — `strings <binary> | grep -o 'assets/index-[A-Za-z0-9_]*\.js'`.
   Abort with a clear message if the frontend was not embedded (guards against
   the blank-screen footgun from a bare `cargo build`).
5. **Install binary** — `install -Dm755 <binary> "$BIN_DIR/webapps"`.
6. **Install icon** — copy `src-tauri/icons/{32x32,64x64,128x128}.png` and
   `128x128@2x.png` into `$DATA_DIR/icons/hicolor/<size>/apps/webapps.png`
   (mapping `128x128@2x.png` → `256x256`). Run `gtk-update-icon-cache` if present
   (non-fatal).
7. **Desktop entry** — write `$DATA_DIR/applications/webapps.desktop`:
   - `Exec=webapps` (relies on `$BIN_DIR` being on `PATH`)
   - `Icon=webapps`
   - `StartupWMClass=WebApps`
   - **No** `WEBKIT_DISABLE_*` env vars — the app self-heals DMA-BUF crashes
     internally (commit fef473b) and keeps GPU acceleration where it works.
   Run `update-desktop-database` if present (non-fatal).
8. **PATH check** — warn if `$BIN_DIR` is not on `$PATH`.

## Uninstall flow

1. Running-instance check (same as install; offer to kill).
2. Remove `$BIN_DIR/webapps`, every installed `webapps.png` icon, and the
   desktop entry. Refresh icon + desktop caches.
3. **Preserve** `~/.config/webapps/` user data; print a note that it was kept and
   how to remove it manually.

## Paths (XDG-aware)

- `BIN_DIR`  = `${XDG_BIN_HOME:-$HOME/.local/bin}`
- `DATA_DIR` = `${XDG_DATA_HOME:-$HOME/.local/share}`
- Binary source: `src-tauri/target/release/webapps`

## Conventions

- `set -euo pipefail`.
- Color-coded `info` / `warn` / `err` echo helpers.
- Each non-fatal external tool (`gtk-update-icon-cache`, `update-desktop-database`)
  guarded with a command-exists check.

## Docs

Update CLAUDE.md "Local Installation & Releases" to point at `scripts/install.sh`
as the canonical path (keeping the manual steps as reference).

## Out of scope (YAGNI)

- Multi-distro packaging (deb/rpm/AppImage) — `--no-bundle` stays.
- System-wide install (`/usr/local`) — single-user `~/.local` only.
- Auto-adding `$BIN_DIR` to `PATH` — warn only, don't edit shell rc files.
