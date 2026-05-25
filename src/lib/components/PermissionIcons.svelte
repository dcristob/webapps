<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { activeCaptures, setCapture } from "../stores/permissions";
  import { spaces, loadSpaces } from "../stores/spaces";
  import { setAppPermission, webviewReload } from "../api";

  let activeAppId = $state<string | null>(null);
  let unlistenCap: UnlistenFn | null = null;
  let unlistenActive: UnlistenFn | null = null;
  let unlistenChanged: UnlistenFn | null = null;

  onMount(async () => {
    unlistenCap = await listen<{ app_id: string; kind: "camera" | "microphone"; active: boolean }>(
      "media-capture-changed",
      (event) => {
        setCapture(event.payload.app_id, event.payload.kind, event.payload.active);
      },
    );
    unlistenActive = await listen<string | null>("active-app-changed", (event) => {
      activeAppId = event.payload;
    });
    unlistenChanged = await listen<{ app_id: string }>(
      "media-permission-changed",
      async () => {
        await loadSpaces();
      },
    );
  });

  onDestroy(() => {
    unlistenCap?.();
    unlistenActive?.();
    unlistenChanged?.();
  });

  const currentApp = $derived.by(() => {
    if (!activeAppId) return null;
    for (const sp of $spaces) {
      const app = sp.apps.find((a) => a.id === activeAppId);
      if (app) return { app, spaceId: sp.space.id };
    }
    return null;
  });

  const cameraState = $derived(currentApp?.app.permissions?.camera ?? "ask");
  const micState = $derived(currentApp?.app.permissions?.microphone ?? "ask");
  const captures = $derived(
    $activeCaptures.get(activeAppId ?? "") ?? { camera: false, microphone: false },
  );

  function classFor(state: string, active: boolean): string {
    if (state === "block") return "icon slashed";
    if (state === "ask") return "icon hidden";
    return active ? "icon active" : "icon allowed";
  }

  async function onClick(kind: "camera" | "microphone", state: string) {
    const app = currentApp;
    if (!app) return;
    if (state === "block") {
      await setAppPermission(app.spaceId, app.app.id, kind, "ask");
      await webviewReload();
    } else if (state === "allow") {
      await setAppPermission(app.spaceId, app.app.id, kind, "block");
    }
  }
</script>

{#if currentApp}
  <div class="permission-icons">
    {#if cameraState !== "ask" || captures.camera}
      <button
        class={classFor(cameraState, captures.camera)}
        title={cameraState === "block"
          ? "Camera blocked. Click to allow."
          : captures.camera
            ? "Camera in use. Click to block."
            : "Camera allowed. Click to block."}
        onclick={() => onClick("camera", cameraState)}
        aria-label="Camera permission"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="6" width="14" height="12" rx="2" />
          <path d="M22 8l-6 4 6 4V8z" />
          {#if cameraState === "block"}
            <line x1="3" y1="3" x2="23" y2="21" stroke="currentColor" stroke-width="2" />
          {/if}
        </svg>
      </button>
    {/if}
    {#if micState !== "ask" || captures.microphone}
      <button
        class={classFor(micState, captures.microphone)}
        title={micState === "block"
          ? "Microphone blocked. Click to allow."
          : captures.microphone
            ? "Microphone in use. Click to block."
            : "Microphone allowed. Click to block."}
        onclick={() => onClick("microphone", micState)}
        aria-label="Microphone permission"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="9" y="2" width="6" height="12" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0" />
          <line x1="12" y1="18" x2="12" y2="22" />
          {#if micState === "block"}
            <line x1="3" y1="3" x2="23" y2="21" stroke="currentColor" stroke-width="2" />
          {/if}
        </svg>
      </button>
    {/if}
  </div>
{/if}

<style>
  .permission-icons {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .icon {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }
  .icon:hover {
    background: var(--bg-hover, #333);
  }
  .icon.allowed { color: var(--text-secondary, #888); }
  .icon.active  { color: #4a9eff; }
  .icon.slashed { color: var(--text-secondary, #666); }
  .icon.hidden  { display: none; }
</style>
