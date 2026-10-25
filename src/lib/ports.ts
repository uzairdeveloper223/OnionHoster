import { invoke } from "@tauri-apps/api/core";

export interface DetectedPort {
  port: number;
  scheme: "http" | "https";
}

export function detectLocalPorts() {
  return invoke<DetectedPort[]>("detect_local_ports");
}
