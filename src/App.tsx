import { useState } from "react";
import { ActivityView } from "@/components/activity/ActivityView";
import { AppShell } from "@/components/layout/AppShell";
import { ServicesView } from "@/components/services/ServicesView";
import { SettingsView } from "@/components/settings/SettingsView";
import { TunnelsView } from "@/components/tunnels/TunnelsView";
import { useTheme } from "@/hooks/useTheme";
import type { View } from "@/types";

export default function App() {
  const [view, setView] = useState<View>("services");
  const { theme, setTheme, toggleTheme } = useTheme();

  return (
    <AppShell
      view={view}
      onViewChange={setView}
      theme={theme}
      onToggleTheme={toggleTheme}
    >
      {view === "services" ? <ServicesView /> : null}
      {view === "tunnels" ? <TunnelsView /> : null}
      {view === "activity" ? <ActivityView /> : null}
      {view === "settings" ? (
        <SettingsView theme={theme} onThemeChange={setTheme} />
      ) : null}
    </AppShell>
  );
}
