import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig } from "../types";
import * as api from "../api";

export const activeAppId = writable<string | null>(null);
export const notificationBadges = writable<Record<string, number>>({});

export async function addNewApp(spaceId: string, name: string, url: string) {
  const app = await api.addApp(spaceId, name, url);
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
  return app;
}

export async function openExistingApp(spaceId: string, appId: string) {
  await api.openApp(spaceId, appId);
  activeAppId.set(appId);
}

export async function switchToExistingApp(spaceId: string, appId: string) {
  await api.switchToApp(spaceId, appId);
  activeAppId.set(appId);
}

export async function closeExistingApp(appId: string) {
  await api.closeApp(appId);
  activeAppId.set(null);
}

export async function removeExistingApp(spaceId: string, appId: string, deleteData: boolean) {
  await api.removeApp(spaceId, appId, deleteData);
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
  activeAppId.set(null);
}

export async function reorderExistingApps(spaceId: string, appIds: string[]) {
  await api.reorderApps(spaceId, appIds);
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
}

export function updateBadge(appId: string, count: number) {
  notificationBadges.update((badges) => ({ ...badges, [appId]: count }));
}

export async function initTitleListener() {
  await listen<{ app_id: string; title: string; badge: number }>(
    "title-changed",
    (event) => {
      updateBadge(event.payload.app_id, event.payload.badge);
    }
  );
}
