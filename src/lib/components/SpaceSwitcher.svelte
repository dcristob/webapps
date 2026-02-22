<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { spaces, activeSpaceId, switchToSpace } from "../stores/spaces";
  import { showDialog } from "../api";

  async function handleSwitchSpace(spaceId: string) {
    await switchToSpace(spaceId);
    await emit("space-switched");
  }

  async function handleCreateSpace() {
    await showDialog("create-space");
  }
</script>

<div class="space-bar">
  {#each $spaces as space (space.space.id)}
    <button
      class="space-icon"
      class:active={$activeSpaceId === space.space.id}
      onclick={() => handleSwitchSpace(space.space.id)}
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
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-secondary, #888);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: background 0.15s, border-color 0.15s;
  }
  .space-icon:hover {
    background: var(--bg-hover, #333);
    color: var(--text-primary, #fff);
  }
  .space-icon.active {
    border-color: var(--accent, #4a9eff);
    background: var(--bg-active, #444);
    color: var(--text-primary, #fff);
  }
  .add-btn {
    border: 2px dashed var(--border-color, #444);
    background: transparent;
    font-size: 15px;
  }
  .add-btn:hover {
    border-color: var(--accent, #4a9eff);
    color: var(--accent, #4a9eff);
  }
</style>
