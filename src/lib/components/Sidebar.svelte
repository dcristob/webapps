<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import AppItem from "./AppItem.svelte";
  import { activeSpace, activeSpaceId, loadSpaces } from "../stores/spaces";
  import { openExistingApp, removeExistingApp, reorderExistingApps } from "../stores/apps";
  import { showDialog, showAppContextMenu } from "../api";
  import type { AppConfig } from "../types";

  let unlisteners: UnlistenFn[] = [];

  onMount(async () => {
    unlisteners.push(
      await listen("dialog-result", async () => { await loadSpaces(); }),
      await listen("space-switched", async () => { await loadSpaces(); }),
      await listen<{ space_id: string; app_id: string }>(
        "context-menu-remove-app",
        async (event) => {
          await removeExistingApp(event.payload.space_id, event.payload.app_id, false);
        }
      ),
    );
  });

  onDestroy(() => {
    unlisteners.forEach((fn) => fn());
  });

  async function handleOpenAddApp() {
    await showDialog("add-app", $activeSpaceId);
  }

  async function handleSelectApp(app: AppConfig) {
    await openExistingApp($activeSpaceId, app.id);
  }

  async function handleContextMenu(app: AppConfig, _event: MouseEvent) {
    await showAppContextMenu($activeSpaceId, app.id);
  }

  async function handleDrop(event: DragEvent) {
    const draggedId = event.dataTransfer?.getData("text/plain");
    if (!draggedId || !$activeSpace) return;
    const appList = event.currentTarget as HTMLElement;
    const items = appList.querySelectorAll(".app-item");
    const currentIds = $activeSpace.apps.map((a) => a.id);
    const draggedIndex = currentIds.indexOf(draggedId);
    let dropIndex = currentIds.length;
    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect();
      if (event.clientY < rect.top + rect.height / 2) {
        dropIndex = i;
        break;
      }
    }
    const newOrder = [...currentIds];
    newOrder.splice(draggedIndex, 1);
    newOrder.splice(dropIndex > draggedIndex ? dropIndex - 1 : dropIndex, 0, draggedId);
    await reorderExistingApps($activeSpaceId, newOrder);
  }
</script>

<div class="sidebar">
  <div
    class="app-list"
    role="list"
    ondragover={(e) => e.preventDefault()}
    ondrop={(e) => { e.preventDefault(); handleDrop(e); }}
  >
    {#if $activeSpace}
      {#each $activeSpace.apps as app (app.id)}
        <AppItem
          {app}
          onSelect={handleSelectApp}
          onContextMenu={handleContextMenu}
        />
      {/each}
    {/if}
  </div>

  <div class="sidebar-footer">
    <button class="add-app-btn" onclick={handleOpenAddApp} title="Add App">+</button>
  </div>
</div>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100%;
    background: var(--bg-primary, #1a1a1a);
    color: var(--text-primary, #ccc);
    overflow: hidden;
  }
  .app-list { flex: 1; overflow-y: auto; padding: 4px; }
  .sidebar-footer {
    padding: 8px;
    border-top: 1px solid var(--border-color, #333);
    display: flex;
    justify-content: center;
  }
  .add-app-btn {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary, #888);
    border: 2px dashed var(--border-color, #444);
    cursor: pointer;
    font-size: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .add-app-btn:hover {
    border-color: var(--accent, #4a9eff);
    color: var(--accent, #4a9eff);
  }
</style>
