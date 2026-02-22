<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { createSpace, closeDialog } from "../api";

  let name = $state("");

  async function handleSubmit() {
    if (name.trim()) {
      await createSpace(name.trim());
      await emit("dialog-result", { type: "space-created" });
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
  <h3>New Space</h3>
  <label>
    Name
    <input bind:value={name} placeholder="Space name..." onkeydown={(e) => e.key === "Enter" && handleSubmit()} autofocus />
  </label>
  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="create" onclick={handleSubmit} disabled={!name.trim()}>Create</button>
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
