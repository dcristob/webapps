import { writable, derived } from "svelte/store";
import type { SpaceConfig } from "../types";
import * as api from "../api";

export const spaces = writable<SpaceConfig[]>([]);
export const activeSpaceId = writable<string>("general");

export const activeSpace = derived(
  [spaces, activeSpaceId],
  ([$spaces, $activeSpaceId]) =>
    $spaces.find((s) => s.space.id === $activeSpaceId) ?? null
);

export async function loadSpaces() {
  const data = await api.listSpaces();
  spaces.set(data);
  const active = await api.getActiveSpace();
  activeSpaceId.set(active);
}

export async function createNewSpace(name: string) {
  const space = await api.createSpace(name);
  spaces.update((s) => [...s, space]);
}

export async function switchToSpace(spaceId: string) {
  await api.switchSpace(spaceId);
  activeSpaceId.set(spaceId);
  await api.hideAllAppWebviews();
  // On entering the space, open the last-used app (this session) or the first
  // app, so something is focused and shortcuts work immediately.
  await api.restoreOrOpenApp();
}

export async function deleteExistingSpace(spaceId: string) {
  await api.deleteSpace(spaceId);
  spaces.update((s) => s.filter((sp) => sp.space.id !== spaceId));
  activeSpaceId.set("general");
}

export async function renameExistingSpace(spaceId: string, newName: string) {
  await api.renameSpace(spaceId, newName);
  spaces.update((s) =>
    s.map((sp) =>
      sp.space.id === spaceId
        ? { ...sp, space: { ...sp.space, name: newName } }
        : sp
    )
  );
}

export async function editExistingSpace(spaceId: string, updates: { name?: string; color?: string }) {
  await api.editSpace(spaceId, updates);
  spaces.update((s) =>
    s.map((sp) =>
      sp.space.id === spaceId
        ? { ...sp, space: { ...sp.space, ...updates } }
        : sp
    )
  );
}

export async function reorderExistingSpaces(spaceIds: string[]) {
  await api.reorderSpaces(spaceIds);
  spaces.update((s) => {
    const sorted = [...s];
    sorted.sort((a, b) => {
      const ai = spaceIds.indexOf(a.space.id);
      const bi = spaceIds.indexOf(b.space.id);
      return (ai === -1 ? Infinity : ai) - (bi === -1 ? Infinity : bi);
    });
    return sorted;
  });
}
