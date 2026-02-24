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

## Usage

1. Create a Space from the dropdown menu
2. Add apps by clicking the "+" button and entering URLs
3. Click on apps in the sidebar to switch between them
4. Drag and drop apps to reorder them
5. Right-click apps to edit or remove them

## Configuration

Configuration is stored in `~/.config/webapps/` in TOML format.

## Roadmap

- [x] Core app with Spaces and webview management
- [ ] Bitwarden CLI integration for credential autofill
- [ ] WebAuthn/Passkey bridge support

## License

MIT
