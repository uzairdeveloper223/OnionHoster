import {
  Moon,
  Network,
  ScrollText,
  Server,
  Settings,
  Sun,
  type LucideIcon,
} from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/StatusDot";
import { APP_VERSION } from "@/lib/app";
import { cn } from "@/lib/utils";
import type { Theme, View } from "@/types";

const navigation: { id: View; label: string; icon: LucideIcon }[] = [
  { id: "services", label: "Services", icon: Server },
  { id: "tunnels", label: "Tunnels", icon: Network },
  { id: "activity", label: "Activity", icon: ScrollText },
  { id: "settings", label: "Settings", icon: Settings },
];

export function Sidebar({
  view,
  onViewChange,
  theme,
  onToggleTheme,
}: {
  view: View;
  onViewChange: (view: View) => void;
  theme: Theme;
  onToggleTheme: () => void;
}) {
  return (
    <aside className="flex w-[248px] shrink-0 flex-col border-r border-line bg-surface">
      <div className="flex h-16 items-center gap-2.5 border-b border-line px-5">
        <div className="flex size-8 items-center justify-center rounded-lg border border-line bg-bg">
          <OnionMark className="size-5" />
        </div>
        <div className="min-w-0">
          <div className="truncate text-[13px] font-semibold tracking-tight">
            OnionHoster
          </div>
          <div className="font-mono text-[10px] text-faint">{APP_VERSION}</div>
        </div>
      </div>

      <nav className="px-3 pt-5" aria-label="Primary">
        <div className="eyebrow px-2.5 pb-2">Workspace</div>
        <div className="space-y-0.5">
          {navigation.map((item) => {
            const Icon = item.icon;
            const active = view === item.id;

            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onViewChange(item.id)}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "flex h-9 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] transition-colors",
                  active
                    ? "bg-hover font-medium text-ink"
                    : "text-muted hover:bg-hover/60 hover:text-ink",
                )}
              >
                <Icon className="size-4" strokeWidth={1.6} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </div>
      </nav>

      <div className="mt-auto space-y-2 p-3">
        <div className="rounded-lg border border-line bg-bg p-3">
          <div className="flex items-center gap-2">
            <StatusDot status="offline" />
            <span className="text-xs font-medium text-ink">Tor is stopped</span>
          </div>
          <p className="mt-1.5 text-[11px] leading-4 text-faint">
            Runtime setup starts in the next step.
          </p>
        </div>

        <div className="flex items-center justify-between rounded-lg border border-line px-2.5 py-1.5">
          <span className="text-[11px] text-muted">
            {theme === "dark" ? "Dark theme" : "Light theme"}
          </span>
          <Button
            variant="ghost"
            size="icon"
            onClick={onToggleTheme}
            aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
          >
            {theme === "dark" ? (
              <Sun className="size-4" strokeWidth={1.6} />
            ) : (
              <Moon className="size-4" strokeWidth={1.6} />
            )}
          </Button>
        </div>
      </div>
    </aside>
  );
}
