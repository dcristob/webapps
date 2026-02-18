<script lang="ts">
  import { spaces, activeSpaceId, switchToSpace, createNewSpace } from "../stores/spaces";

  let showCreateInput = $state(false);
  let newSpaceName = $state("");

  async function handleCreate() {
    if (newSpaceName.trim()) {
      await createNewSpace(newSpaceName.trim());
      newSpaceName = "";
      showCreateInput = false;
    }
  }
</script>

<div class="space-switcher">
  <select
    value={$activeSpaceId}
    onchange={(e) => switchToSpace((e.target as HTMLSelectElement).value)}
  >
    {#each $spaces as space}
      <option value={space.space.id}>{space.space.name}</option>
    {/each}
  </select>

  <button class="add-space-btn" onclick={() => (showCreateInput = !showCreateInput)} title="New Space">
    +
  </button>

  {#if showCreateInput}
    <div class="create-space-input">
      <input
        bind:value={newSpaceName}
        placeholder="Space name..."
        onkeydown={(e) => e.key === "Enter" && handleCreate()}
      />
      <button onclick={handleCreate}>Create</button>
    </div>
  {/if}
</div>

<style>
  .space-switcher {
    padding: 8px;
    border-bottom: 1px solid var(--border-color, #333);
  }
  select {
    width: calc(100% - 36px);
    padding: 6px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
  }
  .add-space-btn {
    width: 28px;
    height: 28px;
    margin-left: 4px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
    cursor: pointer;
  }
  .create-space-input {
    display: flex;
    gap: 4px;
    margin-top: 6px;
  }
  .create-space-input input {
    flex: 1;
    padding: 4px 6px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
  }
  .create-space-input button {
    padding: 4px 8px;
    background: var(--accent, #4a9eff);
    color: #fff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
