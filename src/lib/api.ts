import { invoke } from "@tauri-apps/api/core";
import type { SpaceConfig, AppConfig, AppPermissions, MediaKind, PermissionState } from "./types";

// Space commands
export async function listSpaces(): Promise<SpaceConfig[]> {
  return invoke("list_spaces");
}

export async function getActiveSpace(): Promise<string> {
  return invoke("get_active_space");
}

export async function createSpace(name: string, color?: string): Promise<SpaceConfig> {
  return invoke("create_space", { name, color: color ?? null });
}

export async function renameSpace(spaceId: string, newName: string): Promise<void> {
  return invoke("rename_space", { spaceId, newName });
}

export async function editSpace(spaceId: string, updates: { name?: string; color?: string }): Promise<void> {
  return invoke("edit_space", { spaceId, name: updates.name ?? null, color: updates.color ?? null });
}

export async function reorderSpaces(spaceIds: string[]): Promise<void> {
  return invoke("reorder_spaces", { spaceIds });
}

export async function showSpaceContextMenu(spaceId: string, x: number, y: number): Promise<void> {
  return invoke("show_space_context_menu", { spaceId, x, y });
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
export async function addApp(spaceId: string, name: string, url: string, icon?: string): Promise<AppConfig> {
  return invoke("add_app", { spaceId, name, url, icon: icon ?? null });
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

export async function showAppContextMenu(spaceId: string, appId: string, x: number, y: number): Promise<void> {
  return invoke("show_app_context_menu", { spaceId, appId, x, y });
}

// Sleep
export async function getSleptApps(): Promise<string[]> {
  return invoke("get_slept_apps");
}

// Navigation
export async function webviewGoBack(): Promise<void> {
  return invoke("webview_go_back");
}

export async function webviewReload(): Promise<void> {
  return invoke("webview_reload");
}

// Favicon
export async function fetchSiteInfo(url: string): Promise<[string, string]> {
  return invoke("fetch_site_info", { url });
}

// Dialog
export async function showDialog(dialogType: string, spaceId?: string, params?: Record<string, string>): Promise<void> {
  return invoke("show_dialog", { dialogType, spaceId: spaceId ?? null, params: params ?? null });
}

export async function closeDialog(): Promise<void> {
  return invoke("close_dialog");
}

// Media permissions
export async function setAppPermission(
  spaceId: string,
  appId: string,
  kind: MediaKind,
  stateValue: PermissionState,
): Promise<AppPermissions> {
  return invoke("set_app_permission", {
    spaceId,
    appId,
    kind,
    stateValue,
  });
}

export async function getAppPermissions(
  spaceId: string,
  appId: string,
): Promise<AppPermissions> {
  return invoke("get_app_permissions", { spaceId, appId });
}

export async function respondMediaPermission(
  spaceId: string,
  appId: string,
  camera: PermissionState | null,
  microphone: PermissionState | null,
): Promise<AppPermissions> {
  return invoke("respond_media_permission", {
    spaceId,
    appId,
    camera,
    microphone,
  });
}
