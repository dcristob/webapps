<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { editApp, fetchSiteInfo, closeDialog } from "../api";

  let { spaceId, appId, initialName, initialUrl, initialIcon }: {
    spaceId: string;
    appId: string;
    initialName: string;
    initialUrl: string;
    initialIcon: string;
  } = $props();

  let name = $state(initialName);
  let url = $state(initialUrl);
  let icon = $state(initialIcon);
  let fetchingFavicon = $state(false);

  let iconPreviewSrc = $derived(
    icon && icon !== "auto"
      ? (icon.startsWith("/") ? convertFileSrc(icon) : icon)
      : null
  );

  async function handleChooseIcon() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "ico", "webp"] }],
    });
    if (selected) {
      icon = selected;
    }
  }

  async function handleRefetchFavicon() {
    if (!url.trim()) return;
    fetchingFavicon = true;
    try {
      const [, fetchedIcon] = await fetchSiteInfo(url.trim());
      icon = fetchedIcon;
    } catch {
      // Keep current icon on error
    }
    fetchingFavicon = false;
  }

  async function handleSave() {
    if (!name.trim()) return;
    await editApp(spaceId, appId, {
      name: name.trim(),
      url: url.trim() || undefined,
      icon: icon || undefined,
    });
    await emit("dialog-result", { type: "app-edited" });
    await closeDialog();
  }

  async function handleCancel() {
    await closeDialog();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") handleCancel();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="dialog">
  <h3>Edit App</h3>

  <label>
    Name
    <input bind:value={name} placeholder="App name" onkeydown={(e) => e.key === "Enter" && handleSave()} autofocus />
  </label>

  <label>
    URL
    <input bind:value={url} placeholder="https://example.com" onkeydown={(e) => e.key === "Enter" && handleSave()} />
  </label>

  <div class="icon-section">
    <span class="icon-label">Icon</span>
    <div class="icon-row">
      <div class="icon-preview">
        {#if iconPreviewSrc}
          <img src={iconPreviewSrc} alt="" width="32" height="32" />
        {:else}
          <span class="icon-placeholder">{name.charAt(0).toUpperCase()}</span>
        {/if}
      </div>
      <button class="icon-btn" onclick={handleChooseIcon}>Choose file...</button>
      <button class="icon-btn" onclick={handleRefetchFavicon} disabled={fetchingFavicon}>
        {fetchingFavicon ? "..." : "Re-fetch"}
      </button>
    </div>
  </div>

  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="save" onclick={handleSave} disabled={!name.trim()}>Save</button>
  </div>
</div>

<style>
  .dialog {
    padding: 24px;
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary, #1a1a1a);
    border: 1px solid var(--border-color, #444);
    border-radius: 8px;
  }
  h3 { margin: 0 0 16px; color: var(--text-primary, #fff); font-size: 16px; }
  label { display: block; margin-bottom: 12px; color: var(--text-secondary, #aaa); font-size: 13px; }
  input {
    display: block; width: 100%; margin-top: 4px; padding: 8px;
    background: var(--bg-secondary, #2a2a2a); color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444); border-radius: 4px; box-sizing: border-box;
  }
  .icon-section { margin-bottom: 12px; }
  .icon-label { color: var(--text-secondary, #aaa); font-size: 13px; }
  .icon-row { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
  .icon-preview {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
  }
  .icon-preview img { width: 32px; height: 32px; border-radius: 6px; }
  .icon-placeholder {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    background: var(--accent, #4a9eff); color: #fff;
    border-radius: 8px; font-size: 16px; font-weight: 600;
  }
  .icon-btn {
    padding: 6px 12px; background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #ccc); border: 1px solid var(--border-color, #444);
    border-radius: 4px; cursor: pointer; font-size: 12px; white-space: nowrap;
  }
  .icon-btn:hover { border-color: var(--accent, #4a9eff); }
  .icon-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; padding-top: 16px; }
  .cancel {
    padding: 8px 16px; background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border-color, #444); border-radius: 4px; cursor: pointer;
  }
  .save {
    padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer;
  }
  .save:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
