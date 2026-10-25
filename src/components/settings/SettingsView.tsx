import type { ReactNode } from "react";
import { CircleAlert, Moon, Play, Square, Sun } from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";
import { Panel } from "@/components/ui/Panel";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { StatusDot } from "@/components/ui/StatusDot";
import { APP_VERSION } from "@/lib/app";
import { torLabel, torTone } from "@/lib/tor";
import { cn } from "@/lib/utils";
import { useTorStore } from "@/stores/tor";
import type { Theme } from "@/types";

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
  const running =
    status.state === "starting" ||
    status.state === "bootstrapping" ||
    status.state === "ready";
  const bridgeCount = bridgeLines
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#")).length;

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
            Bridge lines stay in memory for this session. Stop Tor before changing
            them.
          </p>
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
          OnionHoster is independent and is not affiliated with the Tor Project, ngrok,
          Cloudflare, or any content hosted by its users. You are responsible for the
          services you publish and for complying with applicable laws.
        </p>
      </Panel>
    </div>
  );
}
