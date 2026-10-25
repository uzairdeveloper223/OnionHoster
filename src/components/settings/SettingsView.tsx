import type { ReactNode } from "react";
import { Moon, Sun } from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Panel } from "@/components/ui/Panel";
import { APP_VERSION } from "@/lib/app";
import { cn } from "@/lib/utils";
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

      <Panel title="Tor" description="Runtime configuration will be available with the Tor integration.">
        <div className="divide-y divide-line">
          <SettingRow label="Runtime">
            <Value>Bundled Tor</Value>
          </SettingRow>
          <SettingRow label="Connection">
            <Value>Not configured</Value>
          </SettingRow>
          <SettingRow label="Bridges">
            <Value>Direct</Value>
          </SettingRow>
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
