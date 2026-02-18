<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import { loadSpaces } from "./lib/stores/spaces";
  import { initTitleListener } from "./lib/stores/apps";

  onMount(async () => {
    await loadSpaces();
    await initTitleListener();
  });
</script>

<main>
  <Sidebar />
</main>

<style>
  :root {
    --bg-primary: #1a1a1a;
    --bg-secondary: #2a2a2a;
    --bg-hover: #333;
    --bg-active: #444;
    --text-primary: #e0e0e0;
    --text-secondary: #888;
    --border-color: #333;
    --accent: #4a9eff;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  main {
    height: 100vh;
    overflow: hidden;
  }
</style>
