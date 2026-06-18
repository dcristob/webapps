<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { addApp, editApp, fetchSiteInfo, refetchAppIcon, closeDialog } from "../api";
  import { autofocus } from "../actions";

  let { mode, spaceId, appId, initialName, initialUrl, initialIcon }: {
    mode: "add" | "edit";
    spaceId: string;
    appId?: string;
    initialName?: string;
    initialUrl?: string;
    initialIcon?: string;
  } = $props();

  // Form fields are seeded from props once; the dialog is recreated on each
  // open, so capturing the initial value (rather than staying reactive) is
  // the intended behaviour.
  // svelte-ignore state_referenced_locally
  let url = $state(initialUrl ?? "");
  // svelte-ignore state_referenced_locally
  let name = $state(initialName ?? "");
  // svelte-ignore state_referenced_locally
  let icon = $state(initialIcon ?? "auto");
  let loading = $state(false);
  // svelte-ignore state_referenced_locally
  let fetched = $state(mode === "edit");

  let iconPreviewSrc = $derived(
    icon && icon !== "auto"
      ? (icon.startsWith("/") ? convertFileSrc(icon) : icon)
      : null
  );

  async function handleFetchInfo() {
    if (!url.trim()) return;
    let normalizedUrl = url.trim();
    if (!normalizedUrl.startsWith("http://") && !normalizedUrl.startsWith("https://")) {
      normalizedUrl = "https://" + normalizedUrl;
      url = normalizedUrl;
    }
    loading = true;
    try {
      const [title, fetchedIcon] = await fetchSiteInfo(normalizedUrl);
      name = title;
      icon = fetchedIcon;
      fetched = true;
    } catch {
      try {
        const parsed = new URL(normalizedUrl);
        name = parsed.hostname;
      } catch {
        name = normalizedUrl;
      }
      fetched = true;
    }
    loading = false;
  }

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
    loading = true;
    try {
      if (mode === "edit" && appId) {
        // Capture the favicon from the live (authenticated) webview, auto-
        // opening the app if needed. Returns the new local icon path.
        const fetchedIcon = await refetchAppIcon(spaceId, appId);
        icon = fetchedIcon;
      } else {
        // Add mode: no app/webview exists yet, so use the generic fetch.
        const [, fetchedIcon] = await fetchSiteInfo(url.trim());
        icon = fetchedIcon;
      }
    } catch {
      // Keep current icon on error
    }
    loading = false;
  }

  async function handleSubmit() {
    if (!name.trim() || !url.trim()) return;
    if (mode === "add") {
      await addApp(spaceId, name.trim(), url.trim(), icon !== "auto" ? icon : undefined);
      await emit("dialog-result", { type: "app-added" });
    } else {
      await editApp(spaceId, appId!, {
        name: name.trim(),
        url: url.trim(),
        icon: icon || undefined,
      });
      await emit("dialog-result", { type: "app-edited" });
    }
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
  <h3>{mode === "add" ? "Add App" : "Edit App"}</h3>

  <label>
    URL
    <div class="url-row">
      <input bind:value={url} placeholder="https://example.com" onkeydown={(e) => e.key === "Enter" && (fetched ? handleSubmit() : handleFetchInfo())} use:autofocus={mode === "add"} />
      <button onclick={handleFetchInfo} disabled={loading}>{loading ? "..." : "Fetch"}</button>
    </div>
  </label>

  {#if fetched}
    <label>
      Name
      <input bind:value={name} placeholder="App name" onkeydown={(e) => e.key === "Enter" && handleSubmit()} use:autofocus={mode === "edit"} />
    </label>

    <div class="icon-section">
      <span class="icon-label">Icon</span>
      <div class="icon-row">
        <div class="icon-preview">
          {#if iconPreviewSrc}
            <img src={iconPreviewSrc} alt="" width="32" height="32" />
          {:else}
            <span class="icon-placeholder">{(name || "?").charAt(0).toUpperCase()}</span>
          {/if}
        </div>
        <button class="icon-btn" onclick={handleChooseIcon}>Choose file...</button>
        <button class="icon-btn" onclick={handleRefetchFavicon} disabled={loading}>
          {loading ? "..." : "Re-fetch"}
        </button>
      </div>
    </div>
  {/if}

  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="submit" onclick={handleSubmit} disabled={!fetched || !name.trim()}>
      {mode === "add" ? "Add" : "Save"}
    </button>
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
  .url-row { display: flex; gap: 6px; }
  .url-row input { flex: 1; }
  .url-row button {
    padding: 8px 12px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer; white-space: nowrap;
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
  .submit {
    padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer;
  }
  .submit:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
