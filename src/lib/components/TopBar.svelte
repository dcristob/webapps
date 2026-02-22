<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import SpaceSwitcher from "./SpaceSwitcher.svelte";
  import { loadSpaces } from "../stores/spaces";
  import { webviewGoBack, webviewReload } from "../api";

  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    await loadSpaces();
    unlisten = await listen("dialog-result", async () => {
      await loadSpaces();
    });
  });

  onDestroy(() => {
    unlisten?.();
  });
</script>

<div class="topbar">
  <SpaceSwitcher />
  <div class="nav-buttons">
    <button class="nav-btn" onclick={() => webviewGoBack()} title="Go back">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
        <path d="M11 1L4 8l7 7" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </button>
    <button class="nav-btn" onclick={() => webviewReload()} title="Reload">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
        <path d="M13.65 2.35A7.96 7.96 0 0 0 8 0a8 8 0 1 0 8 8h-2a6 6 0 1 1-1.76-4.24L9 7h7V0l-2.35 2.35z"/>
      </svg>
    </button>
  </div>
</div>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 100vh;
    background: var(--bg-primary, #1a1a1a);
    border-bottom: 1px solid var(--border-color, #333);
    overflow: hidden;
  }
  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 12px;
  }
  .nav-btn {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--text-secondary, #888);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }
  .nav-btn:hover {
    background: var(--bg-hover, #333);
    color: var(--text-primary, #e0e0e0);
  }
  .nav-btn:active {
    background: var(--bg-active, #444);
  }
</style>
