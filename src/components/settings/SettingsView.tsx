import { useState, type ReactNode } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Check,
  CircleAlert,
  Download,
  Moon,
  Play,
  RefreshCw,
  ShieldCheck,
  Square,
  Sun,
  Upload,
} from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";
import { Panel } from "@/components/ui/Panel";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { StatusDot } from "@/components/ui/StatusDot";
import { APP_VERSION } from "@/lib/app";
import { exportBackup, importBackup } from "@/lib/backup";
import { torLabel, torTone } from "@/lib/tor";
import { cn } from "@/lib/utils";
import { useRuntimeStore } from "@/stores/runtime";
import { useServiceStore } from "@/stores/services";
import { useTorStore } from "@/stores/tor";
import { useTunnelStore } from "@/stores/tunnels";
import { useUpdateStore } from "@/stores/updates";
import type { Theme } from "@/types";

const inputClass =
  "h-9 w-full rounded-md border border-line bg-bg px-2.5 text-[13px] text-ink outline-none transition-colors placeholder:text-faint focus:border-line-strong disabled:opacity-45";

function passphraseError(value: string): string | null {
  if ([...value].length < 8) {
    return "Use a passphrase with at least 8 characters.";
  }
  return null;
}

function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-3 first:pt-0 last:pb-0">
      <span className="text-sm text-muted">{label}</span>
      <div className="min-w-0 text-right">{children}</div>
    </div>
  );
}

function Value({ children }: { children: ReactNode }) {
  return <span className="font-mono text-[11px] text-ink">{children}</span>;
}

export function SettingsView({
  theme,
  onThemeChange,
}: {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}) {
  const status = useTorStore((state) => state.status);
  const busy = useTorStore((state) => state.busy);
  const startTor = useTorStore((state) => state.start);
  const stopTor = useTorStore((state) => state.stop);
  const bridgeLines = useTorStore((state) => state.bridgeLines);
  const setBridgeLines = useTorStore((state) => state.setBridgeLines);
  const runtimeStatus = useRuntimeStore((state) => state.status);
  const runtimeBusy = useRuntimeStore((state) => state.busy);
  const runtimeError = useRuntimeStore((state) => state.error);
  const repairRuntime = useRuntimeStore((state) => state.repair);
  const updateAvailable = useUpdateStore((state) => state.available);
  const updateChecking = useUpdateStore((state) => state.checking);
  const updateInstalling = useUpdateStore((state) => state.installing);
  const updateProgress = useUpdateStore((state) => state.progress);
  const updateError = useUpdateStore((state) => state.error);
  const checkUpdates = useUpdateStore((state) => state.check);
  const installUpdate = useUpdateStore((state) => state.install);
  const running =
    status.state === "starting" ||
    status.state === "bootstrapping" ||
    status.state === "ready";
  const bridgeCount = bridgeLines
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#")).length;

  const [exportPassphrase, setExportPassphrase] = useState("");
  const [exportConfirm, setExportConfirm] = useState("");
  const [restorePassphrase, setRestorePassphrase] = useState("");
  const [pendingImport, setPendingImport] = useState<string | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupNotice, setBackupNotice] = useState<string | null>(null);

  async function handleExport() {
    setBackupError(null);
    setBackupNotice(null);

    const invalid = passphraseError(exportPassphrase);
    if (invalid) {
      setBackupError(invalid);
      return;
    }
    if (exportPassphrase !== exportConfirm) {
      setBackupError("The passphrases do not match.");
      return;
    }

    try {
      const path = await save({
        title: "Save encrypted OnionHoster backup",
        defaultPath: `onionhoster-backup-${new Date().toISOString().slice(0, 10)}.onionhoster`,
        filters: [
          { name: "OnionHoster backup", extensions: ["onionhoster"] },
        ],
      });
      if (!path) {
        return;
      }

      setBackupBusy(true);
      const summary = await exportBackup(path, exportPassphrase);
      setBackupNotice(
        `Encrypted backup saved with ${summary.services} ${
          summary.services === 1 ? "service" : "services"
        }.`,
      );
      setExportPassphrase("");
      setExportConfirm("");
    } catch (error) {
      setBackupError(String(error));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleChooseRestore() {
    setBackupError(null);
    setBackupNotice(null);

    try {
      const selected = await open({
        title: "Open encrypted OnionHoster backup",
        multiple: false,
        directory: false,
        filters: [
          { name: "OnionHoster backup", extensions: ["onionhoster"] },
        ],
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (path) {
        setPendingImport(path);
        setRestorePassphrase("");
      }
    } catch (error) {
      setBackupError(String(error));
    }
  }

  async function handleRestore() {
    if (!pendingImport) {
      return;
    }

    setBackupError(null);
    setBackupNotice(null);
    const invalid = passphraseError(restorePassphrase);
    if (invalid) {
      setBackupError(invalid);
      return;
    }

    setBackupBusy(true);
    try {
      const summary = await importBackup(pendingImport, restorePassphrase);
      await Promise.all([
        useServiceStore.getState().refresh(),
        useTunnelStore.getState().refresh(),
      ]);
      setBackupNotice(
        `Restored ${summary.services} ${
          summary.services === 1 ? "service" : "services"
        }. Chat and file data were not included.`,
      );
      setPendingImport(null);
      setRestorePassphrase("");
    } catch (error) {
      setBackupError(String(error));
    } finally {
      setBackupBusy(false);
    }
  }

  return (
    <div className="mx-auto w-full max-w-3xl space-y-4 px-6 py-7">
      <Panel title="Appearance" description="The interface stays monochrome in both themes.">
        <div className="divide-y divide-line">
          <SettingRow label="Theme">
            <div
              className="inline-flex rounded-md border border-line bg-bg p-0.5"
              role="group"
              aria-label="Theme"
            >
              <button
                type="button"
                onClick={() => onThemeChange("light")}
                aria-pressed={theme === "light"}
                className={cn(
                  "inline-flex h-7 items-center gap-1.5 rounded px-2.5 text-[11px] transition-colors",
                  theme === "light"
                    ? "bg-ink text-bg"
                    : "text-muted hover:text-ink",
                )}
              >
                <Sun className="size-3.5" strokeWidth={1.6} />
                Light
              </button>
              <button
                type="button"
                onClick={() => onThemeChange("dark")}
                aria-pressed={theme === "dark"}
                className={cn(
                  "inline-flex h-7 items-center gap-1.5 rounded px-2.5 text-[11px] transition-colors",
                  theme === "dark"
                    ? "bg-ink text-bg"
                    : "text-muted hover:text-ink",
                )}
              >
                <Moon className="size-3.5" strokeWidth={1.6} />
                Dark
              </button>
            </div>
          </SettingRow>
        </div>
      </Panel>

      <Panel
        title="Runtime"
        description="Bundled components used by OnionHoster. Repair only copies files; it does not install distro packages."
        actions={
          <Button
            variant="secondary"
            size="sm"
            disabled={runtimeBusy}
            onClick={() => void repairRuntime()}
          >
            <RefreshCw
              className={runtimeBusy ? "size-3.5 animate-spin" : "size-3.5"}
              strokeWidth={1.8}
            />
            {runtimeBusy ? "Repairing" : "Repair"}
          </Button>
        }
      >
        <div className="divide-y divide-line">
          {(runtimeStatus?.components ?? []).map((component) => (
            <SettingRow key={component.name} label={component.name}>
              <span className="inline-flex items-center gap-2">
                <StatusDot status={component.installed ? "online" : "error"} />
                <Value>
                  {component.installed
                    ? component.version ?? "installed"
                    : "missing"}
                </Value>
              </span>
            </SettingRow>
          ))}
          {!runtimeStatus ? (
            <SettingRow label="Status">
              <Value>Checking</Value>
            </SettingRow>
          ) : null}
        </div>
        {runtimeError ? (
          <p className="mt-3 rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
            {runtimeError}
          </p>
        ) : (
          <p className="mt-3 text-[11px] leading-4 text-faint">
            {runtimeStatus?.appimage
              ? "The runtime is stored in the app data directory so it survives in-place updates."
              : "The runtime is bundled with this OnionHoster build and managed by the application."}
          </p>
        )}
      </Panel>

      <Panel
        title="Updates"
        description="Check GitHub releases for a signed OnionHoster update."
        actions={
          <Button
            variant="secondary"
            size="sm"
            disabled={updateChecking || updateInstalling}
            onClick={() => void checkUpdates()}
          >
            <RefreshCw
              className={updateChecking ? "size-3.5 animate-spin" : "size-3.5"}
              strokeWidth={1.8}
            />
            {updateChecking ? "Checking" : "Check now"}
          </Button>
        }
      >
        <div className="divide-y divide-line">
          <SettingRow label="Current version">
            <Value>{APP_VERSION}</Value>
          </SettingRow>
          <SettingRow label="Available version">
            <Value>
              {updateAvailable
                ? updateAvailable.version
                : updateChecking
                  ? "Checking"
                  : "Up to date"}
            </Value>
          </SettingRow>
        </div>

        {updateAvailable ? (
          <div className="mt-4 rounded-lg border border-line bg-bg p-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-xs font-medium text-ink">
                  OnionHoster {updateAvailable.version}
                </p>
                <p className="mt-1 text-[11px] leading-4 text-muted">
                  You are running {updateAvailable.currentVersion}. The update is
                  verified with the OnionHoster signing key before installation.
                </p>
              </div>
              {runtimeStatus?.appimage ? (
                <Button
                  variant="primary"
                  size="sm"
                  disabled={updateInstalling}
                  onClick={() => void installUpdate()}
                >
                  <Download className="size-3.5" strokeWidth={1.6} />
                  {updateInstalling
                    ? updateProgress === null
                      ? "Installing"
                      : `${updateProgress}%`
                    : "Install update"}
                </Button>
              ) : null}
            </div>
            {updateAvailable.body ? (
              <p className="mt-3 max-h-40 overflow-y-auto whitespace-pre-wrap border-t border-line pt-3 text-[11px] leading-5 text-muted">
                {updateAvailable.body}
              </p>
            ) : null}
            {!runtimeStatus?.appimage ? (
              <p className="mt-3 border-t border-line pt-3 text-[11px] leading-4 text-faint">
                Download the new package for your install format from the
                project releases.
              </p>
            ) : null}
          </div>
        ) : null}

        {updateError ? (
          <p
            role="alert"
            className="mt-3 rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted"
          >
            {updateError}
          </p>
        ) : null}

        <p className="mt-3 text-[11px] leading-4 text-faint">
          Update checks and downloads use a normal HTTPS connection to GitHub.
          Downloaded updates are rejected if their signature does not match.
        </p>
      </Panel>

      <Panel
        title="Tor"
        description="Start the bundled Tor runtime and optionally connect through bridges."
        actions={
          <Button
            variant={running ? "secondary" : "primary"}
            size="sm"
            disabled={busy}
            onClick={() => void (running ? stopTor() : startTor())}
          >
            {running ? (
              <Square className="size-3" strokeWidth={1.8} />
            ) : (
              <Play className="size-3" strokeWidth={1.8} />
            )}
            {running ? "Stop Tor" : "Start Tor"}
          </Button>
        }
      >
        <div className="divide-y divide-line">
          <SettingRow label="Runtime">
            <Value>{status.version ? `Tor ${status.version}` : "Bundled Tor"}</Value>
          </SettingRow>
          <SettingRow label="Connection">
            <span className="inline-flex items-center gap-2 font-mono text-[11px] text-ink">
              <StatusDot status={torTone(status)} />
              {torLabel(status)}
            </span>
          </SettingRow>
          <SettingRow label="Bridges">
            <Value>
              {bridgeCount === 0
                ? "Direct"
                : `${bridgeCount} ${bridgeCount === 1 ? "line" : "lines"}`}
            </Value>
          </SettingRow>
        </div>

        {status.state === "starting" || status.state === "bootstrapping" ? (
          <div className="mt-4 rounded-lg border border-line bg-bg p-3">
            <div className="flex items-center justify-between gap-3 text-[11px]">
              <span className="text-muted">Bootstrapping the Tor network</span>
              <span className="font-mono text-ink">{status.bootstrap}%</span>
            </div>
            <ProgressBar value={status.bootstrap} className="mt-2" />
            <p className="mt-2 text-[11px] leading-4 text-faint">{status.summary}</p>
          </div>
        ) : null}

        {status.state === "error" ? (
          <div className="mt-4 flex gap-2.5 rounded-lg border border-line bg-bg p-3">
            <CircleAlert className="mt-0.5 size-4 shrink-0 text-ink" strokeWidth={1.6} />
            <div className="min-w-0">
              <p className="text-xs font-medium text-ink">Tor stopped with an error</p>
              <p className="mt-1 text-[11px] leading-4 text-muted">
                {status.error ?? "The Tor runtime stopped unexpectedly."}
              </p>
            </div>
          </div>
        ) : null}

        <div className="mt-4 border-t border-line pt-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <label htmlFor="tor-bridges" className="text-xs font-medium text-ink">
                Bridges
              </label>
              <p className="mt-1 text-[11px] leading-4 text-faint">
                One bridge per line. Leave empty for a direct connection.
              </p>
            </div>
            <span className="shrink-0 font-mono text-[10px] text-faint">
              {bridgeCount === 0 ? "direct" : `${bridgeCount} configured`}
            </span>
          </div>
          <textarea
            id="tor-bridges"
            value={bridgeLines}
            onChange={(event) => setBridgeLines(event.target.value)}
            disabled={running || busy}
            rows={4}
            spellCheck={false}
            placeholder={"obfs4 192.0.2.1:443 ...\nsnowflake 192.0.2.2:80 ..."}
            className="mt-3 w-full resize-y rounded-lg border border-line bg-bg px-3 py-2.5 font-mono text-[11px] leading-5 text-ink outline-none transition-colors placeholder:text-faint focus:border-line-strong disabled:opacity-45"
          />
          <p className="mt-2 text-[11px] leading-4 text-faint">
            Bridge lines are saved on this device. Stop Tor before changing them.
          </p>
        </div>
      </Panel>

      <Panel
        title="Backup & restore"
        description="Move your onion addresses and service settings to another machine."
      >
        <div className="space-y-4">
          <div className="flex gap-2.5 rounded-lg border border-line bg-bg p-3">
            <ShieldCheck
              className="mt-0.5 size-4 shrink-0 text-ink"
              strokeWidth={1.6}
            />
            <div className="min-w-0">
              <p className="text-xs font-medium text-ink">
                Encrypted .onionhoster backup
              </p>
              <p className="mt-1 text-[11px] leading-4 text-muted">
                Includes onion addresses, private keys, and service settings.
                Chat messages and shared files are not included. Keep the
                passphrase somewhere safe; it cannot be recovered.
              </p>
            </div>
          </div>

          <div>
            <h3 className="text-xs font-medium text-ink">Export a backup</h3>
            <p className="mt-1 text-[11px] leading-4 text-faint">
              Choose a new passphrase for this file. Restoring will need it.
            </p>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              <label className="block">
                <span className="mb-1.5 block text-[11px] font-medium text-muted">
                  Passphrase
                </span>
                <input
                  type="password"
                  value={exportPassphrase}
                  onChange={(event) => setExportPassphrase(event.target.value)}
                  autoComplete="new-password"
                  disabled={backupBusy}
                  className={inputClass}
                />
              </label>
              <label className="block">
                <span className="mb-1.5 block text-[11px] font-medium text-muted">
                  Confirm passphrase
                </span>
                <input
                  type="password"
                  value={exportConfirm}
                  onChange={(event) => setExportConfirm(event.target.value)}
                  autoComplete="new-password"
                  disabled={backupBusy}
                  className={inputClass}
                />
              </label>
            </div>
            <div className="mt-3 flex justify-end">
              <Button
                variant="primary"
                size="sm"
                disabled={backupBusy}
                onClick={() => void handleExport()}
              >
                <Download className="size-3.5" strokeWidth={1.6} />
                Export backup
              </Button>
            </div>
          </div>

          <div className="border-t border-line pt-4">
            <h3 className="text-xs font-medium text-ink">Restore a backup</h3>
            <p className="mt-1 text-[11px] leading-4 text-faint">
              Restoring replaces the current service list. Running services are
              stopped first. Chat and file data stay on this machine.
            </p>
            <div className="mt-3 flex justify-end">
              <Button
                variant="secondary"
                size="sm"
                disabled={backupBusy}
                onClick={() => void handleChooseRestore()}
              >
                <Upload className="size-3.5" strokeWidth={1.6} />
                Choose backup file
              </Button>
            </div>

            {pendingImport ? (
              <div className="mt-3 rounded-lg border border-line bg-bg p-3">
                <p className="truncate font-mono text-[11px] text-ink">
                  {fileName(pendingImport)}
                </p>
                <p className="mt-1 text-[11px] leading-4 text-muted">
                  This will replace every current service and its onion address.
                  Enter the passphrase that was used when this backup was
                  created.
                </p>
                <label className="mt-3 block">
                  <span className="mb-1.5 block text-[11px] font-medium text-muted">
                    Backup passphrase
                  </span>
                  <input
                    type="password"
                    value={restorePassphrase}
                    onChange={(event) =>
                      setRestorePassphrase(event.target.value)
                    }
                    autoComplete="current-password"
                    disabled={backupBusy}
                    autoFocus
                    className={inputClass}
                  />
                </label>
                <div className="mt-3 flex justify-end gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={backupBusy}
                    onClick={() => {
                      setPendingImport(null);
                      setRestorePassphrase("");
                    }}
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="primary"
                    size="sm"
                    disabled={backupBusy || restorePassphrase.length === 0}
                    onClick={() => void handleRestore()}
                  >
                    Restore backup
                  </Button>
                </div>
              </div>
            ) : null}
          </div>

          {backupError ? (
            <div
              role="alert"
              className="flex gap-2.5 rounded-lg border border-line bg-bg p-3"
            >
              <CircleAlert
                className="mt-0.5 size-4 shrink-0 text-ink"
                strokeWidth={1.6}
              />
              <p className="min-w-0 text-[11px] leading-4 text-muted">
                {backupError}
              </p>
            </div>
          ) : null}

          {backupNotice ? (
            <div
              role="status"
              className="flex gap-2.5 rounded-lg border border-line bg-bg p-3"
            >
              <Check
                className="mt-0.5 size-4 shrink-0 text-ink"
                strokeWidth={1.6}
              />
              <p className="min-w-0 text-[11px] leading-4 text-muted">
                {backupNotice}
              </p>
            </div>
          ) : null}
        </div>
      </Panel>

      <Panel title="Updates" description="No public release exists yet.">
        <div className="divide-y divide-line">
          <SettingRow label="Automatic updates">
            <Value>After first release</Value>
          </SettingRow>
          <SettingRow label="Channel">
            <Value>Stable</Value>
          </SettingRow>
        </div>
      </Panel>

      <Panel title="About" description="Project and build information.">
        <div className="flex items-start gap-3">
          <div className="flex size-10 items-center justify-center rounded-xl border border-line bg-bg">
            <OnionMark className="size-5" />
          </div>
          <div className="min-w-0">
            <div className="text-sm font-medium text-ink">OnionHoster</div>
            <div className="mt-0.5 font-mono text-[11px] text-faint">
              {APP_VERSION} / GPL-3.0-or-later
            </div>
          </div>
        </div>

        <p className="mt-4 text-xs leading-5 text-muted">
          A local desktop control panel for publishing local sites and TCP services
          through Tor.
        </p>

        <div className="mt-4 space-y-2 border-t border-line pt-4 font-mono text-[11px]">
          <div className="flex items-baseline justify-between gap-4">
            <span className="text-faint">GitHub</span>
            <span className="truncate text-muted">github.com/uzairdeveloper223</span>
          </div>
          <div className="flex items-baseline justify-between gap-4">
            <span className="text-faint">LinkedIn</span>
            <span className="truncate text-muted">linkedin.com/in/uzairmughal001</span>
          </div>
          <div className="flex items-baseline justify-between gap-4">
            <span className="text-faint">Website</span>
            <span className="truncate text-muted">uzair.is-a.dev</span>
          </div>
          <div className="flex items-baseline justify-between gap-4">
            <span className="text-faint">Platform</span>
            <span className="truncate text-muted">Linux preview</span>
          </div>
        </div>

        <p className="mt-4 border-t border-line pt-4 text-[11px] leading-5 text-faint">
          OnionHoster is independent and is not affiliated with the Tor Project,
          Cloudflare, or any content hosted by its users. You are responsible for the
          services you publish and for complying with applicable laws.
        </p>
      </Panel>
    </div>
  );
}
