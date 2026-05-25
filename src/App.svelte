<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import AppDialog from "./lib/components/AppDialog.svelte";
  import SpaceDialog from "./lib/components/SpaceDialog.svelte";
  import { loadSpaces } from "./lib/stores/spaces";
  import { initTitleListener } from "./lib/stores/apps";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { pendingRequest, setCapture } from "./lib/stores/permissions";

  const params = new URLSearchParams(window.location.search);
  const mode = params.get("mode");
  const dialogMode = params.get("dialog");
  const dialogSpaceId = params.get("spaceId") ?? "";
  const dialogAppId = params.get("appId") ?? "";
  const dialogAppName = decodeURIComponent(params.get("name") ?? "");
  const dialogAppUrl = decodeURIComponent(params.get("url") ?? "");
  const dialogAppIcon = decodeURIComponent(params.get("icon") ?? "auto");
  const dialogSpaceName = decodeURIComponent(params.get("spaceName") ?? "");
  const dialogSpaceColor = decodeURIComponent(params.get("spaceColor") ?? "#4a9eff");

  let unlistenReq: UnlistenFn | null = null;
  let unlistenCap: UnlistenFn | null = null;
  let unlistenChanged: UnlistenFn | null = null;

  onMount(async () => {
    // Sidebar mode: load spaces and init title listener
    if (!dialogMode && !mode) {
      await loadSpaces();
      await initTitleListener();

      unlistenReq = await listen<{
        space_id: string;
        app_id: string;
        camera: boolean;
        microphone: boolean;
      }>("media-permission-request", (event) => {
        pendingRequest.set({
          spaceId: event.payload.space_id,
          appId: event.payload.app_id,
          camera: event.payload.camera,
          microphone: event.payload.microphone,
        });
      });

      unlistenCap = await listen<{
        app_id: string;
        kind: "camera" | "microphone";
        active: boolean;
      }>("media-capture-changed", (event) => {
        setCapture(event.payload.app_id, event.payload.kind, event.payload.active);
      });

      unlistenChanged = await listen<{ app_id: string }>(
        "media-permission-changed",
        async () => {
          await loadSpaces();
        },
      );
    }
    // Topbar mode: handled by TopBar component itself
    // Dialog mode: no init needed
  });

  onDestroy(() => {
    unlistenReq?.();
    unlistenCap?.();
    unlistenChanged?.();
  });
</script>

{#if dialogMode === "add-app"}
  <AppDialog mode="add" spaceId={dialogSpaceId} />
{:else if dialogMode === "edit-app"}
  <AppDialog
    mode="edit"
    spaceId={dialogSpaceId}
    appId={dialogAppId}
    initialName={dialogAppName}
    initialUrl={dialogAppUrl}
    initialIcon={dialogAppIcon}
  />
{:else if dialogMode === "create-space"}
  <SpaceDialog mode="create" />
{:else if dialogMode === "edit-space"}
  <SpaceDialog
    mode="edit"
    spaceId={dialogSpaceId}
    initialName={dialogSpaceName}
    initialColor={dialogSpaceColor}
  />
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
