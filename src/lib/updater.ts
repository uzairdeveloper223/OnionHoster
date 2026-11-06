import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { invoke } from "@tauri-apps/api/core";
import { APP_VERSION, isAndroid, isTauri } from "@/lib/app";

export interface UpdateInfo {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
  percent: number | null;
}

let pendingUpdate: Update | null = null;
let pendingAndroidUrl: string | null = null;

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri) {
    return null;
  }

  await closePendingUpdate();
  if (isAndroid) {
    const manifest = await invoke<{
      version?: string;
      url?: string;
      date?: string;
      body?: string;
    }>("get_android_update");
    if (!manifest.version) {
      return null;
    }
    if (!/^\d+\.\d+\.\d+$/.test(manifest.version) ||
        manifest.url !== `https://github.com/uzairdeveloper223/OnionHoster/releases/download/v${manifest.version}/OnionHoster_${manifest.version}_android_arm64.apk`) {
      throw new Error("The Android update manifest is invalid.");
    }
    const current = APP_VERSION.split(".").map(Number);
    const next = manifest.version.split(".").map(Number);
    const different = next.findIndex((part, index) => part !== current[index]);
    if (different < 0 || next[different] < current[different]) {
      return null;
    }
    pendingAndroidUrl = manifest.url;
    return {
      currentVersion: APP_VERSION,
      version: manifest.version,
      date: manifest.date ?? null,
      body: manifest.body ?? null,
    };
  }
  const update = await check({ timeout: 20_000 });
  if (!update) {
    return null;
  }

  pendingUpdate = update;
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date ?? null,
    body: update.body ?? null,
  };
}

export async function installPendingUpdate(
  onProgress: (progress: UpdateProgress) => void,
) {
  if (isAndroid && pendingAndroidUrl) {
    await invoke("open_android_release", { url: pendingAndroidUrl });
    return;
  }
  if (!pendingUpdate) {
    throw new Error("Check for an update before installing it.");
  }

  let downloaded = 0;
  let total: number | null = null;

  await pendingUpdate.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength ?? null;
      onProgress({ downloaded, total, percent: total ? 0 : null });
      return;
    }

    if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress({
        downloaded,
        total,
        percent: total ? Math.min(100, Math.round((downloaded / total) * 100)) : null,
      });
    }
  });

  await relaunch();
}

export async function closePendingUpdate() {
  pendingAndroidUrl = null;
  if (!pendingUpdate) {
    return;
  }

  const update = pendingUpdate;
  pendingUpdate = null;
  try {
    await update.close();
  } catch {
    // The resource may already be closed after an install.
  }
}
