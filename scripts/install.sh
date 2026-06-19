#!/usr/bin/env bash
#
# install.sh — build and install WebApps for the current user.
#
#   ./scripts/install.sh              build + install (default)
#   ./scripts/install.sh --skip-build install an already-built binary
#   ./scripts/install.sh --uninstall  remove binary, icons, desktop entry
#   ./scripts/install.sh --help       show usage
#
set -euo pipefail

# --- paths -----------------------------------------------------------------
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BINARY_SRC="$REPO_ROOT/src-tauri/target/release/webapps"
BINARY_DST="$BIN_DIR/webapps"
DESKTOP_FILE="$DATA_DIR/applications/webapps.desktop"
ICON_DIR="$DATA_DIR/icons/hicolor"

# icon source basename -> hicolor size directory
ICON_SIZES=("32x32:32x32" "64x64:64x64" "128x128:128x128" "128x128@2x:256x256")

# --- output helpers --------------------------------------------------------
if [[ -t 1 ]]; then
  C_INFO="$(printf '\033[1;34m')"; C_WARN="$(printf '\033[1;33m')"
  C_ERR="$(printf '\033[1;31m')";  C_OK="$(printf '\033[1;32m')"
  C_RST="$(printf '\033[0m')"
else
  C_INFO=""; C_WARN=""; C_ERR=""; C_OK=""; C_RST=""
fi
info() { printf '%s==>%s %s\n' "$C_INFO" "$C_RST" "$*"; }
ok()   { printf '%s==>%s %s\n' "$C_OK"   "$C_RST" "$*"; }
warn() { printf '%swarning:%s %s\n' "$C_WARN" "$C_RST" "$*" >&2; }
err()  { printf '%serror:%s %s\n'   "$C_ERR"  "$C_RST" "$*" >&2; }
die()  { err "$*"; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  sed -n '3,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

# --- shared steps ----------------------------------------------------------

# Offer to kill a running webapps instance; abort if user declines.
check_running_instance() {
  local pids
  if ! pids="$(pgrep -x webapps 2>/dev/null)"; then
    return 0
  fi
  warn "WebApps is running (PID(s): $(echo "$pids" | tr '\n' ' '))."
  warn "Two instances sharing ~/.config/webapps/webview-data/ corrupt each"
  warn "other's WebKit state (cookies/IndexedDB)."
  read -r -p "Kill the running instance now? [y/N] " reply
  case "$reply" in
    [yY]|[yY][eE][sS])
      # shellcheck disable=SC2086
      kill $pids 2>/dev/null || true
      sleep 1
      if pgrep -x webapps >/dev/null 2>&1; then
        # shellcheck disable=SC2046
        kill -9 $(pgrep -x webapps) 2>/dev/null || true
      fi
      ok "Stopped running instance."
      ;;
    *)
      die "Aborted: close WebApps before installing."
      ;;
  esac
}

refresh_caches() {
  if have gtk-update-icon-cache; then
    gtk-update-icon-cache -q -t -f "$ICON_DIR" 2>/dev/null || true
  fi
  if have update-desktop-database; then
    update-desktop-database -q "$DATA_DIR/applications" 2>/dev/null || true
  fi
}

# --- install ---------------------------------------------------------------
do_install() {
  local skip_build="$1"

  have npm || die "npm not found on PATH."
  if [[ "$skip_build" == "no" ]]; then
    have cargo || die "cargo not found on PATH."
  fi

  check_running_instance

  if [[ "$skip_build" == "no" ]]; then
    info "Building production binary (npm run tauri -- build --no-bundle)..."
    ( cd "$REPO_ROOT" && npm run tauri -- build --no-bundle )
  else
    info "Skipping build (--skip-build)."
  fi

  [[ -f "$BINARY_SRC" ]] || die "Binary not found: $BINARY_SRC (build first?)"

  info "Verifying the frontend was embedded..."
  # Note: no `grep -q` — it exits on first match, sending SIGPIPE to `strings`,
  # which `set -o pipefail` would then report as a pipeline failure.
  if ! strings "$BINARY_SRC" | grep -o 'assets/index-[A-Za-z0-9_]*\.js' >/dev/null; then
    die "Frontend assets not embedded in the binary. Did you build via the Tauri CLI? A bare 'cargo build' will not embed the frontend and the app shows a blank screen."
  fi
  ok "Frontend embedded."

  info "Installing binary -> $BINARY_DST"
  install -Dm755 "$BINARY_SRC" "$BINARY_DST"

  info "Installing icons -> $ICON_DIR"
  local entry src size dst
  for entry in "${ICON_SIZES[@]}"; do
    src="${entry%%:*}"; size="${entry##*:}"
    if [[ -f "$REPO_ROOT/src-tauri/icons/$src.png" ]]; then
      dst="$ICON_DIR/$size/apps/webapps.png"
      install -Dm644 "$REPO_ROOT/src-tauri/icons/$src.png" "$dst"
    fi
  done

  info "Writing desktop entry -> $DESKTOP_FILE"
  mkdir -p "$(dirname "$DESKTOP_FILE")"
  cat >"$DESKTOP_FILE" <<'EOF'
[Desktop Entry]
Name=WebApps
Comment=Turn web apps into standalone desktop experiences
Exec=webapps
Icon=webapps
Type=Application
Categories=Network;WebBrowser;Utility;
StartupWMClass=WebApps
Terminal=false
EOF
  chmod 644 "$DESKTOP_FILE"

  refresh_caches

  ok "WebApps installed."
  case ":$PATH:" in
    *":$BIN_DIR:"*) info "Launch with: webapps" ;;
    *) warn "$BIN_DIR is not on your PATH. Add it, or launch via: $BINARY_DST" ;;
  esac
}

# --- uninstall -------------------------------------------------------------
do_uninstall() {
  check_running_instance

  info "Removing binary, icons, and desktop entry..."
  rm -f "$BINARY_DST"
  rm -f "$DESKTOP_FILE"
  local entry size
  for entry in "${ICON_SIZES[@]}"; do
    size="${entry##*:}"
    rm -f "$ICON_DIR/$size/apps/webapps.png"
  done

  refresh_caches

  ok "WebApps uninstalled."
  warn "User data preserved at ~/.config/webapps/ (remove manually if desired)."
}

# --- main ------------------------------------------------------------------
main() {
  local mode="install" skip_build="no"
  for arg in "$@"; do
    case "$arg" in
      --uninstall)  mode="uninstall" ;;
      --skip-build) skip_build="yes" ;;
      -h|--help)    usage; exit 0 ;;
      *) die "Unknown option: $arg (try --help)" ;;
    esac
  done

  case "$mode" in
    install)   do_install "$skip_build" ;;
    uninstall) do_uninstall ;;
  esac
}

main "$@"
