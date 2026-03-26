<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { AppConfig } from "../types";
  import { activeAppId, notificationBadges, sleptApps } from "../stores/apps";

  let { app, onSelect, onContextMenu }: {
    app: AppConfig;
    onSelect: (app: AppConfig) => void;
    onContextMenu: (app: AppConfig, event: MouseEvent) => void;
  } = $props();

  let isActive = $derived($activeAppId === app.id);
  let badge = $derived($notificationBadges[app.id] ?? 0);
  let isSlept = $derived($sleptApps.has(app.id));
  let isDragging = $state(false);

  let iconSrc = $derived(
    app.icon && app.icon !== "auto"
      ? (app.icon.startsWith("/") ? convertFileSrc(app.icon) : app.icon)
      : null
  );
</script>

<button
  class="app-item"
  class:active={isActive}
  class:slept={isSlept}
  class:dragging={isDragging}
  draggable="true"
  onclick={() => onSelect(app)}
  oncontextmenu={(e) => { e.preventDefault(); onContextMenu(app, e); }}
  ondragstart={(e) => {
    e.dataTransfer?.setData("text/plain", app.id);
    isDragging = true;
  }}
  ondragend={() => {
    isDragging = false;
  }}
  title={isSlept ? `${app.name} (sleeping)` : app.name}
>
  <div class="app-icon">
    {#if iconSrc}
      <img src={iconSrc} alt="" width="32" height="32" />
    {:else}
      <span class="icon-placeholder">{app.name.charAt(0).toUpperCase()}</span>
    {/if}
    {#if badge > 0}
      <span class="badge">{badge > 99 ? "99+" : badge}</span>
    {/if}
  </div>
</button>

<style>
  .app-item {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    padding: 6px 0;
    background: transparent;
    color: var(--text-primary, #ccc);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .app-item:hover { background: var(--bg-hover, #333); }
  .app-item.active { background: var(--bg-active, #444); }
  .app-item.slept { opacity: 0.45; }
  .app-item.slept:hover { opacity: 0.7; }
  .app-item.dragging { opacity: 0.4; }
  .app-icon {
    position: relative;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .app-icon img { width: 32px; height: 32px; border-radius: 6px; }
  .icon-placeholder {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent, #4a9eff);
    color: #fff;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
  }
  .badge {
    position: absolute;
    top: -2px;
    right: -2px;
    background: #e74c3c;
    color: #fff;
    font-size: 10px;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 3px;
  }
</style>
