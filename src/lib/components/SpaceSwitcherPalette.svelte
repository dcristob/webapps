<script lang="ts">
  import { onMount } from "svelte";
  import { emit } from "@tauri-apps/api/event";
  import { spaces, activeSpaceId, switchToSpace, loadSpaces } from "../stores/spaces";
  import { focusActiveApp, closeDialog } from "../api";
  import { autofocus } from "../actions";

  let query = $state("");
  let selected = $state(0);

  // The palette runs in its own dialog webview with a fresh (empty) store
  // instance — populate it on mount so the list isn't blank.
  onMount(() => {
    void loadSpaces();
  });

  // Filtered, case-insensitive substring match on space name. Empty query → all.
  let filtered = $derived(
    $spaces.filter((s) =>
      s.space.name.toLowerCase().includes(query.trim().toLowerCase())
    )
  );

  async function activate(spaceId: string) {
    await switchToSpace(spaceId);
    await emit("space-switched");
    await focusActiveApp();
    await closeDialog();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      void closeDialog();
      return;
    }
    if (filtered.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selected = (selected + 1) % filtered.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selected = (selected - 1 + filtered.length) % filtered.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const target = filtered[selected];
      if (target) void activate(target.space.id);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="palette">
  <input
    class="search"
    bind:value={query}
    oninput={() => (selected = 0)}
    placeholder="Switch space…"
    use:autofocus
  />
  <div class="list" role="listbox">
    {#each filtered as space, i (space.space.id)}
      <button
        class="row"
        class:selected={i === selected}
        class:active={$activeSpaceId === space.space.id}
        style="--space-color: {space.space.color}"
        onclick={() => activate(space.space.id)}
        role="option"
        aria-selected={i === selected}
        title={space.space.name}
      >
        <span class="dot"></span>
        <span class="name">{space.space.name}</span>
        <span class="count">{space.apps.length} apps</span>
      </button>
    {:else}
      <div class="empty">No spaces match "{query}".</div>
    {/each}
  </div>
</div>

<style>
  .palette {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-primary, #1a1a1a);
    border: 1px solid var(--border-color, #444);
    border-radius: 8px;
    overflow: hidden;
  }
  .search {
    width: 100%;
    padding: 12px 14px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: none;
    border-bottom: 1px solid var(--border-color, #333);
    font-size: 14px;
    box-sizing: border-box;
    outline: none;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    background: transparent;
    color: var(--text-primary, #e0e0e0);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
  }
  .row:hover,
  .row.selected {
    background: var(--bg-hover, #333);
  }
  .row.active .name {
    font-weight: 600;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--space-color, #4a9eff);
    flex-shrink: 0;
  }
  .name {
    flex: 1;
  }
  .count {
    color: var(--text-secondary, #888);
    font-size: 12px;
  }
  .empty {
    padding: 16px;
    color: var(--text-secondary, #888);
    font-size: 13px;
    text-align: center;
  }
</style>
