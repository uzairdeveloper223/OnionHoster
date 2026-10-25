import { invoke } from "@tauri-apps/api/core";

export interface RuntimeComponent {
  name: string;
  installed: boolean;
  version: string | null;
  path: string | null;
}

export interface RuntimeStatus {
  appimage: boolean;
  ready: boolean;
  components: RuntimeComponent[];
}

export function getRuntimeStatus() {
  return invoke<RuntimeStatus>("get_runtime_status");
}

export function repairRuntime() {
  return invoke<RuntimeStatus>("repair_runtime");
}
