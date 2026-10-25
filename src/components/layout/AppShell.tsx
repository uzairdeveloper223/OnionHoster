import type { ReactNode } from "react";
import { Topbar } from "@/components/layout/Topbar";
import { Sidebar } from "@/components/layout/Sidebar";
import type { Theme, View } from "@/types";

const viewMeta: Record<View, { title: string; subtitle: string }> = {
  services: {
    title: "Services",
    subtitle: "Publish local sites and TCP ports through Tor.",
  },
  tunnels: {
    title: "Tunnels",
    subtitle: "Optional clearnet access through Cloudflare or a custom command.",
  },
  activity: {
    title: "Activity",
    subtitle: "Tor, service, and tunnel events from this machine.",
  },
  settings: {
    title: "Settings",
    subtitle: "Runtime, appearance, updates, and project information.",
  },
};

export function AppShell({
  view,
  onViewChange,
  theme,
  onToggleTheme,
  children,
}: {
  view: View;
  onViewChange: (view: View) => void;
  theme: Theme;
  onToggleTheme: () => void;
  children: ReactNode;
}) {
  return (
    <div className="flex h-screen overflow-hidden bg-bg text-ink">
      <Sidebar
        view={view}
        onViewChange={onViewChange}
        theme={theme}
        onToggleTheme={onToggleTheme}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar title={viewMeta[view].title} subtitle={viewMeta[view].subtitle} />
        <main className="min-h-0 flex-1 overflow-y-auto">{children}</main>
      </div>
    </div>
  );
}
