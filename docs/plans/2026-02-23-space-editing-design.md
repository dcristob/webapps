# Space Editing Feature Design

## Overview
Add the ability to edit space names, change space colors, and delete spaces via a right-click context menu on space icons.

## UX Flow

When users right-click on any space icon in the SpaceSwitcher:
1. A context menu appears at the cursor position
2. Menu contains three sections:
   - **Rename** - Opens inline editing to change the name
   - **Change Color** - Opens a color picker popup with 8 preset colors
   - **Delete** - Shows a confirmation dialog (disabled for "general" space)

### Color Palette
8 preset colors stored as CSS classes:
- gray, blue, green, red, purple, orange, pink, teal

## Data Model Changes

### Backend (Rust)
File: `src-tauri/src/config/models.rs`

```rust
pub struct SpaceInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub isolation: IsolationMode,
    pub color: Option<String>, // NEW: color class name
}
```

### Commands to Update
- `rename_space` - rename to `update_space` to handle name + color
- Add `set_space_color` - standalone color change command
- `delete_space` - already exists

## Frontend Components

### SpaceContextMenu.svelte
- Native HTML `<dialog>` element
- Positioned absolutely at mouse coordinates
- Sections separated by dividers
- Auto-close on click outside or Escape

### SpaceSwitcher.svelte Changes
- Add `@oncontextmenu` handler to space buttons
- Prevent default browser menu
- Dynamic CSS class: `space-{space.color}`
- Pass space ID and coords to context menu

### Color Picker
- Grid of 8 circular color buttons
- Current selection highlighted with white border
- Click applies immediately

### Rename Flow
- Inline input replaces icon temporarily
- Enter to save, Escape to cancel
- Or use small modal dialog

### Delete Flow
- Confirmation modal: "Delete '{name}'? Apps will be lost."
- Cannot delete "general" space
- Falls back to "general" if active space is deleted

## CSS Classes

```css
.space-gray { background: #666; }
.space-blue { background: #4a9eff; }
.space-green { background: #4ade80; }
.space-red { background: #f87171; }
.space-purple { background: #c084fc; }
.space-orange { background: #fb923c; }
.space-pink { background: #f472b6; }
.space-teal { background: #2dd4bf; }
```

## API Changes

### New Commands
- `update_space(space_id, name, color)` - Update space properties
- `set_space_color(space_id, color)` - Quick color change

### Existing Commands
- `delete_space(space_id)` - Already prevents deleting "general"

## File Changes

1. `src-tauri/src/config/models.rs` - Add color field
2. `src-tauri/src/commands/spaces.rs` - Add update_space command
3. `src/lib/components/SpaceContextMenu.svelte` - New component
4. `src/lib/components/SpaceSwitcher.svelte` - Add right-click handler
5. `src/lib/api.ts` - Add update_space wrapper
6. Global styles - Add color CSS classes
