<script lang="ts">
  import type { AppConfig } from "../types";
  import { activeAppId, notificationBadges } from "../stores/apps";

  let { app, onSelect, onContextMenu }: {
    app: AppConfig;
    onSelect: (app: AppConfig) => void;
    onContextMenu: (app: AppConfig, event: MouseEvent) => void;
  } = $props();

  let isActive = $derived($activeAppId === app.id);
  let badge = $derived($notificationBadges[app.id] ?? 0);
  let isDragging = $state(false);
</script>

<button
  class="app-item"
  class:active={isActive}
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
  title={app.url}
>
  <div class="app-icon">
    {#if app.icon && app.icon !== "auto"}
      <img src={app.icon} alt="" width="24" height="24" />
    {:else}
      <span class="icon-placeholder">{app.name.charAt(0).toUpperCase()}</span>
    {/if}
    {#if badge > 0}
      <span class="badge">{badge > 99 ? "99+" : badge}</span>
    {/if}
  </div>
  <span class="app-name">{app.name}</span>
</button>

<style>
  .app-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px;
    background: transparent;
    color: var(--text-primary, #ccc);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
  }
  .app-item:hover { background: var(--bg-hover, #333); }
  .app-item.active { background: var(--bg-active, #444); color: var(--text-primary, #fff); }
  .app-item.dragging { opacity: 0.4; }
  .app-icon {
    position: relative;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .app-icon img { width: 24px; height: 24px; border-radius: 4px; }
  .icon-placeholder {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent, #4a9eff);
    color: #fff;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
  }
  .badge {
    position: absolute;
    top: -4px;
    right: -4px;
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
  .app-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
