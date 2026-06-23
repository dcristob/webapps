# WebApps

A Tauri v2 desktop application that turns web apps into standalone desktop experiences. Organize your web applications into Spaces with configurable session isolation.

## Features

- **Spaces**: Organize web apps into separate workspaces
- **Session Isolation**: Choose between shared or isolated sessions per app
- **Sidebar Interface**: Easy switching between apps with drag-and-drop reordering
- **Badge Notifications**: Visual indicators for unread notifications
- **Custom Icons**: Automatic favicon fetching with fallback icons
- **Lightweight**: Built with Tauri for native performance

## Tech Stack

- **Framework**: Tauri v2
- **Frontend**: Svelte 5 + TypeScript
- **Backend**: Rust
- **Styling**: Tailwind CSS

## Development

### Prerequisites

- Rust
- Node.js
- System dependencies (Linux):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libgtk-3-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Setup

```bash
npm install
```

### Development Mode

```bash
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

## Installation (Linux)

`scripts/install.sh` builds WebApps and installs it for the current user: it
builds via the Tauri CLI, verifies the frontend was embedded, copies the binary
to `~/.local/bin/webapps`, installs the app icons, and writes a desktop entry
so WebApps appears in your application launcher.

```bash
./scripts/install.sh              # build + install (default)
./scripts/install.sh --skip-build # install an already-built binary
./scripts/install.sh --uninstall  # remove binary, icons, and desktop entry
```

The script detects a running WebApps instance and offers to stop it first (two
instances sharing `~/.config/webapps/` can corrupt each other's WebKit state).
Uninstalling leaves your config in `~/.config/webapps/` untouched.

If `~/.local/bin` is not on your `PATH`, add it so the `webapps` command and the
launcher entry resolve.

## Usage

1. Create a Space from the dropdown menu
2. Add apps by clicking the "+" button and entering URLs
3. Click on apps in the sidebar to switch between them
4. Drag and drop apps to reorder them
5. Right-click apps to edit or remove them

## Keyboard Shortcuts

Shortcuts work everywhere — in the sidebar/topbar **and** inside hosted apps. The
shell binds a capture-phase listener, so a shortcut is always handled by WebApps
and never reaches the underlying web app (Slack/Rambox-style).

| Shortcut | Action |
| --- | --- |
| `Ctrl`+`Tab` | Cycle to the next app |
| `Ctrl`+`Shift`+`Tab` | Cycle to the previous app |
| `Ctrl`+`1`…`9` | Jump to the app at that position (no wrap) |
| `Ctrl`+`B` | Toggle the sidebar |
| `Ctrl`+`N` | Add a new app to the current space |
| `Ctrl`+`W` | Sleep the active app and switch to the previous one |
| `Ctrl`+`Shift`+`S` | Open the searchable space switcher |

The sidebar visibility (`Ctrl`+`B`) is persisted across launches.

## Configuration

Configuration is stored in `~/.config/webapps/` in TOML format.

## Troubleshooting

### The app crashes on startup (Linux)

On some Mesa/GPU/Wayland combinations, WebKitGTK's DMA-BUF GPU renderer fails
to allocate buffers at startup — you'll see `Failed to create GBM buffer` and/or
`Error 71 (Protocol error) dispatching to Wayland display`, and the window dies
immediately.

WebApps handles this automatically: it tries the DMA-BUF renderer first, and if
a launch crashes during startup it disables the renderer for subsequent launches
(falling back to CPU/shared-memory rendering). So if it crashes once on first
run, **just launch it again** — it will recover and remember the working setting.

The setting is stored in `~/.config/webapps/dmabuf-mode`. To force a re-probe
(for example after a driver update), delete that file:

```bash
rm ~/.config/webapps/dmabuf-mode
```

You can also override the detection manually with an environment variable:

```bash
# Force the DMA-BUF renderer off for this launch (GPU accel disabled):
WEBAPPS_DISABLE_DMABUF=1 webapps
```

Disabling DMA-BUF trades GPU acceleration for stability; for typical web apps
(Gmail, Outlook, Drive, …) the difference is negligible.

## Roadmap

- [x] Core app with Spaces and webview management
- [ ] Bitwarden CLI integration for credential autofill
- [ ] WebAuthn/Passkey bridge support

## License

MIT
