#!/usr/bin/env bash
#
# bump-version.sh — cut a new WebApps release locally.
#
# Bumps the version in Cargo.toml, package.json, and tauri.conf.json (keeping
# them in sync), refreshes Cargo.lock, regenerates CHANGELOG.md via git-cliff,
# then commits and tags vX.Y.Z. It does NOT push — review the result, then
# `git push --follow-tags origin main` to trigger the release CI.
#
#   ./scripts/bump-version.sh 0.2.0    bump to 0.2.0 (commit + tag, no push)
#   ./scripts/bump-version.sh --help   show usage
#
# Prerequisites: git, git-cliff (https://git-cliff.org). On Arch:
#   paru -S git-cliff-bin   # or: cargo install git-cliff
#
set -euo pipefail

# --- paths ------------------------------------------------------------------
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

CARGO_TOML="src-tauri/Cargo.toml"
PACKAGE_JSON="package.json"
TAURI_CONF="src-tauri/tauri.conf.json"
CARGO_LOCK="src-tauri/Cargo.lock"
CHANGELOG="CHANGELOG.md"

# --- output helpers ---------------------------------------------------------
if [[ -t 1 ]]; then
  C_INFO="$(printf '\033[1;34m')"; C_OK="$(printf '\033[1;32m')"
  C_WARN="$(printf '\033[1;33m')"; C_ERR="$(printf '\033[1;31m')"
  C_RST="$(printf '\033[0m')"
else
  C_INFO=""; C_OK=""; C_WARN=""; C_ERR=""; C_RST=""
fi
info() { printf '%s==>%s %s\n' "$C_INFO" "$C_RST" "$*"; }
ok()   { printf '%s==>%s %s\n' "$C_OK"   "$C_RST" "$*"; }
warn() { printf '%swarning:%s %s\n' "$C_WARN" "$C_RST" "$*" >&2; }
err()  { printf '%serror:%s %s\n'   "$C_ERR"  "$C_RST" "$*" >&2; }
die()  { err "$*"; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() { sed -n '3,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

# --- args -------------------------------------------------------------------
[[ "${1:-}" == "--help" || "${#}" -eq 0 ]] && { usage; exit 0; }
[[ "${#}" -eq 1 ]] || { usage >&2; exit 1; }

NEW="${1#v}" # tolerate a leading "v"
[[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || die "'$NEW' is not a valid semver (MAJOR.MINOR.PATCH)."

# --- preflight --------------------------------------------------------------
have git      || die "git not found on PATH."
have git-cliff || die "git-cliff not found on PATH. Install it (Arch: paru -S git-cliff-bin)."

git rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || die "not inside a git work tree."

# Refuse to bump over a dirty tree — it would mix unrelated changes into the
# release commit.
if ! git diff --quiet || ! git diff --cached --quiet; then
  die "working tree is dirty (unstaged or staged changes). Commit or stash first."
fi

# --- version consistency check ---------------------------------------------
read_current() { # $1 = file, echoes the version string
  case "$1" in
    *.toml) grep -m1 '^version = ' "$1" | sed 's/^version = "\(.*\)"$/\1/' ;;
    *.json) grep -m1 '"version"'   "$1" | sed 's/.*"version": *"\(.*\)".*/\1/' ;;
  esac
}

V_CARGO=$(read_current "$CARGO_TOML")
V_PKG=$(read_current "$PACKAGE_JSON")
V_TF=$(read_current "$TAURI_CONF")
[[ "$V_CARGO" == "$V_PKG" && "$V_CARGO" == "$V_TF" ]] \
  || die "version files drifted: Cargo.toml=$V_CARGO package.json=$V_PKG tauri.conf.json=$V_TF. Reconcile before bumping."

CURRENT="$V_CARGO"
[[ "$NEW" != "$CURRENT" ]] || die "already at $CURRENT; nothing to bump."
[[ -f "$CARGO_LOCK" ]] || die "$CARGO_LOCK not found (expected to be committed)."

info "Bumping $CURRENT -> $NEW"

# --- bump the three version files ------------------------------------------
# Cargo.toml: the package `version =` is the only one at line start (deps are
# indented inside `{ }`), so an anchored substitution is unambiguous.
sed -i "s/^version = \"[^\"]*\"/version = \"$NEW\"/" "$CARGO_TOML"
# JSON files: replace only the first `"version": "..."` occurrence.
sed -i "0,/\"version\": \"[^\"]*\"/{s/\"version\": \"[^\"]*\"/\"version\": \"$NEW\"/}" "$PACKAGE_JSON"
sed -i "0,/\"version\": \"[^\"]*\"/{s/\"version\": \"[^\"]*\"/\"version\": \"$NEW\"/}" "$TAURI_CONF"

# --- refresh Cargo.lock -----------------------------------------------------
# Surgical edit: change ONLY the `webapps` package version (the line right
# after `name = "webapps"`). This deliberately avoids `cargo update`, which
# resolves to the latest compatible versions and would drag dozens of
# unrelated dependency bumps into the release commit.
sed -i '/^name = "webapps"$/{n;s/^version = ".*"/version = "'"$NEW"'"/}' "$CARGO_LOCK" \
  || die "failed to update $CARGO_LOCK."

# --- regenerate CHANGELOG.md -----------------------------------------------
# First release: no tags yet, so --unreleased captures the full history.
# --prepend inserts the new section at the top (git-cliff dedupes the header).
touch "$CHANGELOG"
git cliff --unreleased --tag "v$NEW" --prepend "$CHANGELOG" \
  || die "git-cliff failed to regenerate $CHANGELOG."

# --- commit + tag -----------------------------------------------------------
git add "$CARGO_TOML" "$PACKAGE_JSON" "$TAURI_CONF" "$CARGO_LOCK" "$CHANGELOG"
git commit -q -m "chore(release): v$NEW"
git tag -a "v$NEW" -m "Release v$NEW"

ok "Created release v$NEW (commit + tag, not pushed)."
printf '\n'
info "Next:"
printf '  git push --follow-tags origin %s\n' "$(git rev-parse --abbrev-ref HEAD)"
printf '  # CI builds all platforms and publishes the stable GitHub Release.\n'
