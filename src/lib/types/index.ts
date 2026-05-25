export interface SpaceInfo {
  id: string;
  name: string;
  icon: string;
  color: string;
  isolation: "shared" | "per-app";
}

export type PermissionState = "ask" | "allow" | "block";
export type MediaKind = "camera" | "microphone";

export interface AppPermissions {
  camera: PermissionState;
  microphone: PermissionState;
}

export interface AppConfig {
  id: string;
  name: string;
  url: string;
  icon: string;
  isolation_override: boolean;
  permissions: AppPermissions;
}

export interface SpaceConfig {
  space: SpaceInfo;
  apps: AppConfig[];
}

export interface GeneralSettings {
  sidebar_width: number;
  theme: string;
}

export interface GlobalConfig {
  general: GeneralSettings;
}
