import { writable } from "svelte/store";

export interface PendingRequest {
  spaceId: string;
  appId: string;
  camera: boolean;
  microphone: boolean;
}

export interface CaptureState {
  camera: boolean;
  microphone: boolean;
}

export const pendingRequest = writable<PendingRequest | null>(null);
export const activeCaptures = writable<Map<string, CaptureState>>(new Map());

export function setCapture(appId: string, kind: "camera" | "microphone", active: boolean) {
  activeCaptures.update((m) => {
    const next = new Map(m);
    const cur = next.get(appId) ?? { camera: false, microphone: false };
    const updated = { ...cur, [kind]: active };
    next.set(appId, updated);
    return next;
  });
}

export function clearCaptures(appId: string) {
  activeCaptures.update((m) => {
    const next = new Map(m);
    next.delete(appId);
    return next;
  });
}
