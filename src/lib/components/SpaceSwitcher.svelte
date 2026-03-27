<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { spaces, activeSpaceId, switchToSpace, deleteExistingSpace, reorderExistingSpaces, loadSpaces } from "../stores/spaces";
  import { showDialog, showSpaceContextMenu } from "../api";

  let dragSourceId = $state<string | null>(null);
  let dragOverId = $state<string | null>(null);

  onMount(() => {
    const unlisteners = [
      listen("dialog-result", () => loadSpaces()),
      listen("context-menu-edit-space", (e: any) => {
        const { space_id, name, color } = e.payload;
        showDialog("edit-space", space_id, { spaceName: name, spaceColor: color });
      }),
      listen("context-menu-delete-space", async (e: any) => {
        const { space_id } = e.payload;
        await deleteExistingSpace(space_id);
        await emit("space-switched");
      }),
    ];
    return () => { unlisteners.forEach((p) => p.then((fn) => fn())); };
  });

  async function handleSwitchSpace(spaceId: string) {
    await switchToSpace(spaceId);
    await emit("space-switched");
  }

  async function handleCreateSpace() {
    await showDialog("create-space");
  }

  function handleContextMenu(e: MouseEvent, spaceId: string) {
    e.preventDefault();
    // GTK anchors the menu's bottom-left at the given point, so add enough
    // vertical offset (~100px) to push the visible menu below the topbar.
    showSpaceContextMenu(spaceId, e.clientX + 4, 48 + 70);
  }

  // Drag-drop reordering
  function handleDragStart(e: DragEvent, spaceId: string) {
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", spaceId);
    dragSourceId = spaceId;
  }

  function handleDragOver(e: DragEvent, spaceId: string) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dragOverId = spaceId;
  }

  function handleDragLeave() {
    dragOverId = null;
  }

  function handleDragEnd() {
    dragSourceId = null;
    dragOverId = null;
  }

  async function handleDrop(e: DragEvent, targetId: string) {
    e.preventDefault();
    dragOverId = null;
    const sourceId = e.dataTransfer?.getData("text/plain");
    if (!sourceId || sourceId === targetId) return;

    const ids = $spaces.map((s) => s.space.id);
    const fromIndex = ids.indexOf(sourceId);
    const toIndex = ids.indexOf(targetId);
    if (fromIndex === -1 || toIndex === -1) return;

    ids.splice(fromIndex, 1);
    ids.splice(toIndex, 0, sourceId);

    await reorderExistingSpaces(ids);
  }
</script>

<div class="space-bar">
  {#each $spaces as space (space.space.id)}
    <button
      class="space-icon"
      class:active={$activeSpaceId === space.space.id}
      class:drag-over={dragOverId === space.space.id && dragSourceId !== space.space.id}
      class:dragging={dragSourceId === space.space.id}
      style="--space-color: {space.space.color}"
      onclick={() => handleSwitchSpace(space.space.id)}
      oncontextmenu={(e) => handleContextMenu(e, space.space.id)}
      draggable={true}
      ondragstart={(e) => handleDragStart(e, space.space.id)}
      ondragover={(e) => handleDragOver(e, space.space.id)}
      ondragleave={handleDragLeave}
      ondragend={handleDragEnd}
      ondrop={(e) => handleDrop(e, space.space.id)}
      title={space.space.name}
    >
      {space.space.name.charAt(0).toUpperCase()}
    </button>
  {/each}
  <button class="space-icon add-btn" onclick={handleCreateSpace} title="New Space">+</button>
</div>

<style>
  .space-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    height: 100%;
  }
  .space-icon {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: color-mix(in srgb, var(--space-color) 25%, var(--bg-secondary, #2a2a2a));
    color: var(--space-color);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: background 0.15s, border-color 0.15s, opacity 0.15s, transform 0.15s;
  }
  .space-icon:hover {
    background: color-mix(in srgb, var(--space-color) 35%, var(--bg-hover, #333));
    color: #fff;
  }
  .space-icon.active {
    border-color: var(--space-color);
    background: color-mix(in srgb, var(--space-color) 40%, var(--bg-active, #444));
    color: #fff;
  }
  .space-icon.dragging {
    opacity: 0.4;
  }
  .space-icon.drag-over {
    transform: scale(1.15);
    border-color: var(--space-color);
  }
  .add-btn {
    border: 2px dashed var(--border-color, #444);
    background: transparent;
    color: var(--text-secondary, #888);
    font-size: 15px;
  }
  .add-btn:hover {
    border-color: var(--accent, #4a9eff);
    color: var(--accent, #4a9eff);
  }
</style>
