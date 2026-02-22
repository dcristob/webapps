<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { fetchSiteInfo, addApp, closeDialog } from "../api";

  let { spaceId }: { spaceId: string } = $props();

  let url = $state("");
  let name = $state("");
  let iconPath = $state<string | undefined>(undefined);
  let loading = $state(false);
  let fetched = $state(false);

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
      iconPath = fetchedIcon !== "auto" ? fetchedIcon : undefined;
      fetched = true;
    } catch (e) {
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

  async function handleSubmit() {
    if (url.trim() && name.trim()) {
      await addApp(spaceId, name.trim(), url.trim(), iconPath);
      await emit("dialog-result", { type: "app-added" });
      await closeDialog();
    }
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
  <h3>Add App</h3>
  <label>
    URL
    <div class="url-row">
      <input bind:value={url} placeholder="https://example.com" onkeydown={(e) => e.key === "Enter" && handleFetchInfo()} autofocus />
      <button onclick={handleFetchInfo} disabled={loading}>{loading ? "..." : "Fetch"}</button>
    </div>
  </label>
  {#if fetched}
    <label>
      Name
      <input bind:value={name} placeholder="App name" onkeydown={(e) => e.key === "Enter" && handleSubmit()} />
    </label>
  {/if}
  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="add" onclick={handleSubmit} disabled={!fetched || !name.trim()}>Add</button>
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
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; padding-top: 16px; }
  .cancel {
    padding: 8px 16px; background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border-color, #444); border-radius: 4px; cursor: pointer;
  }
  .add {
    padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer;
  }
  .add:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
