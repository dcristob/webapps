<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import AddAppDialog from "./lib/components/AddAppDialog.svelte";
  import EditAppDialog from "./lib/components/EditAppDialog.svelte";
  import CreateSpaceDialog from "./lib/components/CreateSpaceDialog.svelte";
  import { loadSpaces } from "./lib/stores/spaces";
  import { initTitleListener } from "./lib/stores/apps";

  const params = new URLSearchParams(window.location.search);
  const mode = params.get("mode");
  const dialogMode = params.get("dialog");
  const dialogSpaceId = params.get("spaceId") ?? "";
  const dialogAppId = params.get("appId") ?? "";
  const dialogAppName = decodeURIComponent(params.get("name") ?? "");
  const dialogAppUrl = decodeURIComponent(params.get("url") ?? "");
  const dialogAppIcon = decodeURIComponent(params.get("icon") ?? "auto");

  onMount(async () => {
    // Sidebar mode: load spaces and init title listener
    if (!dialogMode && !mode) {
      await loadSpaces();
      await initTitleListener();
    }
    // Topbar mode: handled by TopBar component itself
    // Dialog mode: no init needed
  });
</script>

{#if dialogMode === "add-app"}
  <AddAppDialog spaceId={dialogSpaceId} />
{:else if dialogMode === "edit-app"}
  <EditAppDialog
    spaceId={dialogSpaceId}
    appId={dialogAppId}
    initialName={dialogAppName}
    initialUrl={dialogAppUrl}
    initialIcon={dialogAppIcon}
  />
{:else if dialogMode === "create-space"}
  <CreateSpaceDialog />
{:else if mode === "topbar"}
  <TopBar />
{:else}
  <main>
    <Sidebar />
  </main>
{/if}

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
