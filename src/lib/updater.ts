import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { isTauri } from "@/lib/app";

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

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri) {
    return null;
  }

  await closePendingUpdate();
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
