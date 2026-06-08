<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { createSpace, editSpace, closeDialog } from "../api";
  import { autofocus } from "../actions";

  interface Props {
    mode: "create" | "edit";
    spaceId?: string;
    initialName?: string;
    initialColor?: string;
  }

  const COLORS = [
    "#4a9eff", "#7c5cfc", "#e04eff", "#ff4a8a",
    "#ff6b4a", "#ff9f1a", "#ffd04a", "#4adf7c",
    "#2ac5a0", "#4acfdf", "#8899aa", "#667788",
  ];

  let { mode, spaceId = "", initialName = "", initialColor = "#4a9eff" }: Props = $props();

  // Seeded from props once; the dialog is recreated on each open, so capturing
  // the initial value (rather than staying reactive) is intended.
  // svelte-ignore state_referenced_locally
  let name = $state(initialName);
  // svelte-ignore state_referenced_locally
  let color = $state(initialColor);

  async function handleSubmit() {
    if (!name.trim()) return;

    if (mode === "create") {
      await createSpace(name.trim(), color);
      await emit("dialog-result", { type: "space-created" });
    } else {
      await editSpace(spaceId, { name: name.trim(), color });
      await emit("dialog-result", { type: "space-edited" });
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
  <h3>{mode === "create" ? "New Space" : "Edit Space"}</h3>

  <label>
    Name
    <input bind:value={name} placeholder="Space name..." onkeydown={(e) => e.key === "Enter" && handleSubmit()} use:autofocus />
  </label>

  <div class="color-section">
    <span class="color-label">Color</span>
    <div class="color-grid">
      {#each COLORS as c (c)}
        <button
          class="color-swatch"
          class:selected={color === c}
          style="background: {c}"
          onclick={() => color = c}
          title={c}
        ></button>
      {/each}
    </div>
  </div>

  <div class="preview">
    <div class="preview-circle" style="background: {color}; border-color: {color}">
      {name.trim() ? name.trim().charAt(0).toUpperCase() : "?"}
    </div>
    <span class="preview-name">{name.trim() || "Space name"}</span>
  </div>

  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="create" onclick={handleSubmit} disabled={!name.trim()}>
      {mode === "create" ? "Create" : "Save"}
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

  .color-section { margin-bottom: 16px; }
  .color-label { display: block; color: var(--text-secondary, #aaa); font-size: 13px; margin-bottom: 8px; }
  .color-grid {
    display: flex; flex-wrap: wrap; gap: 8px;
  }
  .color-swatch {
    width: 28px; height: 28px; border-radius: 50%;
    border: 2px solid transparent; cursor: pointer;
    transition: border-color 0.15s, transform 0.15s;
    padding: 0;
  }
  .color-swatch:hover { transform: scale(1.15); }
  .color-swatch.selected { border-color: #fff; transform: scale(1.15); }

  .preview {
    display: flex; align-items: center; gap: 10px;
    padding: 10px 12px; margin-bottom: 12px;
    background: var(--bg-secondary, #2a2a2a); border-radius: 6px;
  }
  .preview-circle {
    width: 32px; height: 32px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    font-size: 13px; font-weight: 600; color: #fff; flex-shrink: 0;
    border: 2px solid;
  }
  .preview-name { color: var(--text-primary, #fff); font-size: 13px; }

  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; padding-top: 16px; }
  .cancel {
    padding: 8px 16px; background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border-color, #444); border-radius: 4px; cursor: pointer;
  }
  .create {
    padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer;
  }
  .create:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
