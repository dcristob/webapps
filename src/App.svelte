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
  import { evalInApp } from "./lib/api";

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

  const BANNER_JS = (kindsText: string, allowArgs: string, blockArgs: string) => `
(function() {
  var EXISTING = document.getElementById('__webapps_perm_banner');
  if (EXISTING) EXISTING.remove();

  var bar = document.createElement('div');
  bar.id = '__webapps_perm_banner';
  bar.style.cssText = [
    'position:fixed','top:0','left:0','right:0','z-index:2147483647',
    'background:#222','color:#fff','padding:10px 16px',
    'font-family:-apple-system,BlinkMacSystemFont,sans-serif','font-size:14px',
    'display:flex','align-items:center','gap:12px',
    'box-shadow:0 2px 8px rgba(0,0,0,0.3)'
  ].join(';') + ';';
  bar.innerHTML =
    '<span style="flex:1">' + ${JSON.stringify(`This app wants to use your ${kindsText}.`)} + '</span>' +
    '<button id="__webapps_perm_allow" style="background:#4a9eff;color:#fff;border:none;padding:6px 14px;border-radius:4px;cursor:pointer;font-size:14px">Allow</button>' +
    '<button id="__webapps_perm_block" style="background:#444;color:#fff;border:none;padding:6px 14px;border-radius:4px;cursor:pointer;font-size:14px">Block</button>';
  document.documentElement.appendChild(bar);

  document.getElementById('__webapps_perm_allow').addEventListener('click', function() {
    window.__TAURI_INTERNALS__.invoke('respond_media_permission', ${allowArgs});
    bar.remove();
  });
  document.getElementById('__webapps_perm_block').addEventListener('click', function() {
    window.__TAURI_INTERNALS__.invoke('respond_media_permission', ${blockArgs});
    bar.remove();
  });
})();
`;

  $effect(() => {
    const req = $pendingRequest;
    if (!req) return;

    const kinds: string[] = [];
    if (req.camera) kinds.push("camera");
    if (req.microphone) kinds.push("microphone");
    const kindsText = kinds.join(" and ");

    const allowArgs = JSON.stringify({
      spaceId: req.spaceId,
      appId: req.appId,
      camera: req.camera ? "allow" : null,
      microphone: req.microphone ? "allow" : null,
    });
    const blockArgs = JSON.stringify({
      spaceId: req.spaceId,
      appId: req.appId,
      camera: req.camera ? "block" : null,
      microphone: req.microphone ? "block" : null,
    });

    evalInApp(req.appId, BANNER_JS(kindsText, allowArgs, blockArgs)).catch(() => {});
  });

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
          pendingRequest.set(null);
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
