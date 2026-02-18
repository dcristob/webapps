<script lang="ts">
  import SpaceSwitcher from "./SpaceSwitcher.svelte";
  import AppItem from "./AppItem.svelte";
  import AddAppDialog from "./AddAppDialog.svelte";
  import { activeSpace, activeSpaceId } from "../stores/spaces";
  import { addNewApp, openExistingApp, removeExistingApp, reorderExistingApps } from "../stores/apps";
  import type { AppConfig } from "../types";

  let showAddDialog = $state(false);
  let contextMenuApp: AppConfig | null = $state(null);
  let contextMenuPos = $state({ x: 0, y: 0 });

  async function handleAddApp(name: string, url: string) {
    await addNewApp($activeSpaceId, name, url);
    showAddDialog = false;
  }

  async function handleSelectApp(app: AppConfig) {
    await openExistingApp($activeSpaceId, app.id);
  }

  function handleContextMenu(app: AppConfig, event: MouseEvent) {
    contextMenuApp = app;
    contextMenuPos = { x: event.clientX, y: event.clientY };
  }

  async function handleRemoveApp() {
    if (contextMenuApp) {
      await removeExistingApp($activeSpaceId, contextMenuApp.id, false);
      contextMenuApp = null;
    }
  }

  function closeContextMenu() {
    contextMenuApp = null;
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

<svelte:window onclick={closeContextMenu} />

<div class="sidebar">
  <SpaceSwitcher />

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
    <button class="add-app-btn" onclick={() => (showAddDialog = true)}>
      + Add App
    </button>
  </div>
</div>

{#if showAddDialog}
  <AddAppDialog onAdd={handleAddApp} onCancel={() => (showAddDialog = false)} />
{/if}

{#if contextMenuApp}
  <div class="context-menu" style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px">
    <button onclick={handleRemoveApp}>Remove App</button>
  </div>
{/if}

<style>
  .sidebar { display: flex; flex-direction: column; height: 100vh; width: 100%; background: var(--bg-primary, #1a1a1a); color: var(--text-primary, #ccc); overflow: hidden; }
  .app-list { flex: 1; overflow-y: auto; padding: 4px; }
  .sidebar-footer { padding: 8px; border-top: 1px solid var(--border-color, #333); }
  .add-app-btn { width: 100%; padding: 8px; background: transparent; color: var(--text-secondary, #888); border: 1px dashed var(--border-color, #444); border-radius: 6px; cursor: pointer; }
  .add-app-btn:hover { background: var(--bg-hover, #333); color: var(--text-primary, #fff); }
  .context-menu { position: fixed; background: var(--bg-primary, #1e1e1e); border: 1px solid var(--border-color, #444); border-radius: 6px; padding: 4px; z-index: 2000; box-shadow: 0 4px 12px rgba(0,0,0,0.3); }
  .context-menu button { display: block; width: 100%; padding: 6px 12px; background: transparent; color: var(--text-primary, #ccc); border: none; border-radius: 4px; cursor: pointer; text-align: left; }
  .context-menu button:hover { background: var(--bg-hover, #333); }
</style>
