export interface SpaceInfo {
  id: string;
  name: string;
  icon: string;
  isolation: "shared" | "per-app";
}

export interface AppConfig {
  id: string;
  name: string;
  url: string;
  icon: string;
  isolation_override: boolean;
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
