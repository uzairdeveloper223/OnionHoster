import { invoke } from "@tauri-apps/api/core";

export interface BackupSummary {
  services: number;
  createdAt: number;
  path: string;
}

export interface RestoreSummary {
  services: number;
  createdAt: number;
}

export function exportBackup(
  path: string,
  passphrase: string,
): Promise<BackupSummary> {
  return invoke<BackupSummary>("export_backup", { path, passphrase });
}

export function importBackup(
  path: string,
  passphrase: string,
): Promise<RestoreSummary> {
  return invoke<RestoreSummary>("import_backup", { path, passphrase });
}
