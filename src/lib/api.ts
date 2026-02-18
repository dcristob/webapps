import { invoke } from "@tauri-apps/api/core";
import type { SpaceConfig, AppConfig } from "./types";

// Space commands
export async function listSpaces(): Promise<SpaceConfig[]> {
  return invoke("list_spaces");
}

export async function getActiveSpace(): Promise<string> {
  return invoke("get_active_space");
}

export async function createSpace(name: string): Promise<SpaceConfig> {
  return invoke("create_space", { name });
}

export async function renameSpace(spaceId: string, newName: string): Promise<void> {
  return invoke("rename_space", { spaceId, newName });
}

export async function deleteSpace(spaceId: string): Promise<void> {
  return invoke("delete_space", { spaceId });
}

export async function switchSpace(spaceId: string): Promise<SpaceConfig> {
  return invoke("switch_space", { spaceId });
}

export async function setSpaceIsolation(spaceId: string, mode: "shared" | "per-app"): Promise<void> {
  return invoke("set_space_isolation", { spaceId, mode });
}

// App commands
export async function addApp(spaceId: string, name: string, url: string): Promise<AppConfig> {
  return invoke("add_app", { spaceId, name, url });
}

export async function removeApp(spaceId: string, appId: string, deleteData: boolean): Promise<void> {
  return invoke("remove_app", { spaceId, appId, deleteData });
}

export async function editApp(spaceId: string, appId: string, updates: { name?: string; url?: string; icon?: string; isolationOverride?: boolean; }): Promise<AppConfig> {
  return invoke("edit_app", { spaceId, appId, name: updates.name ?? null, url: updates.url ?? null, icon: updates.icon ?? null, isolationOverride: updates.isolationOverride ?? null });
}

export async function reorderApps(spaceId: string, appIds: string[]): Promise<void> {
  return invoke("reorder_apps", { spaceId, appIds });
}

export async function getAppsForSpace(spaceId: string): Promise<AppConfig[]> {
  return invoke("get_apps_for_space", { spaceId });
}

// Webview commands
export async function openApp(spaceId: string, appId: string): Promise<void> {
  return invoke("open_app", { spaceId, appId });
}

export async function switchToApp(spaceId: string, appId: string): Promise<void> {
  return invoke("switch_to_app", { spaceId, appId });
}

export async function closeApp(appId: string): Promise<void> {
  return invoke("close_app", { appId });
}

export async function hideAllAppWebviews(): Promise<void> {
  return invoke("hide_all_app_webviews");
}

export async function getActiveApp(): Promise<string | null> {
  return invoke("get_active_app");
}

// Favicon
export async function fetchSiteInfo(url: string): Promise<[string, string]> {
  return invoke("fetch_site_info", { url });
}
