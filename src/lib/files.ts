import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

export function pickAndroidSource(directory: boolean) {
  return invoke<string | null>("pick_android_source", { directory });
}

export function discardAndroidSource(path: string) {
  return invoke<void>("discard_android_source", { path });
}

export async function saveClientKey(name: string, content: string) {
  const path = await save({
    title: "Save Tor client authorization key",
    defaultPath: name,
    filters: [{ name: "Tor client key", extensions: ["auth_private"] }],
  });
  if (path) {
    await invoke("export_client_key", { path, content });
  }
}
