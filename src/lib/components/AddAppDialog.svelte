<script lang="ts">
  import { fetchSiteInfo } from "../api";

  let { onAdd, onCancel }: {
    onAdd: (name: string, url: string) => void;
    onCancel: () => void;
  } = $props();

  let url = $state("");
  let name = $state("");
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
      const [title, _iconPath] = await fetchSiteInfo(normalizedUrl);
      name = title;
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

  function handleSubmit() {
    if (url.trim() && name.trim()) {
      onAdd(name.trim(), url.trim());
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="dialog-overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onCancel(); }}>
  <div class="dialog">
    <h3>Add App</h3>
    <label>
      URL
      <div class="url-row">
        <input bind:value={url} placeholder="https://example.com" onkeydown={(e) => e.key === "Enter" && handleFetchInfo()} />
        <button onclick={handleFetchInfo} disabled={loading}>{loading ? "..." : "Fetch"}</button>
      </div>
    </label>
    {#if fetched}
      <label>
        Name
        <input bind:value={name} placeholder="App name" />
      </label>
    {/if}
    <div class="actions">
      <button class="cancel" onclick={onCancel}>Cancel</button>
      <button class="add" onclick={handleSubmit} disabled={!fetched || !name.trim()}>Add</button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
  .dialog { background: var(--bg-primary, #1e1e1e); border: 1px solid var(--border-color, #444); border-radius: 8px; padding: 20px; width: 400px; max-width: 90%; }
  h3 { margin: 0 0 16px; color: var(--text-primary, #fff); }
  label { display: block; margin-bottom: 12px; color: var(--text-secondary, #aaa); font-size: 13px; }
  input { display: block; width: 100%; margin-top: 4px; padding: 8px; background: var(--bg-secondary, #2a2a2a); color: var(--text-primary, #fff); border: 1px solid var(--border-color, #444); border-radius: 4px; box-sizing: border-box; }
  .url-row { display: flex; gap: 6px; }
  .url-row input { flex: 1; }
  .url-row button { padding: 8px 12px; background: var(--accent, #4a9eff); color: #fff; border: none; border-radius: 4px; cursor: pointer; white-space: nowrap; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .cancel { padding: 8px 16px; background: transparent; color: var(--text-secondary, #aaa); border: 1px solid var(--border-color, #444); border-radius: 4px; cursor: pointer; }
  .add { padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff; border: none; border-radius: 4px; cursor: pointer; }
  .add:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
