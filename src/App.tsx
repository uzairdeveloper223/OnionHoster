import { useEffect, useState } from "react";
import { ActivityView } from "@/components/activity/ActivityView";
import { AppShell } from "@/components/layout/AppShell";
import { ServicesView } from "@/components/services/ServicesView";
import { SettingsView } from "@/components/settings/SettingsView";
import { RuntimeSetup } from "@/components/setup/RuntimeSetup";
import { TunnelsView } from "@/components/tunnels/TunnelsView";
import { useTheme } from "@/hooks/useTheme";
import { useRuntimeStore } from "@/stores/runtime";
import { useServiceStore } from "@/stores/services";
import { useTorStore } from "@/stores/tor";
import { useTunnelStore } from "@/stores/tunnels";
import { useUpdateStore } from "@/stores/updates";
import type { View } from "@/types";

export default function App() {
  const [view, setView] = useState<View>("services");
  const [setupDismissed, setSetupDismissed] = useState(false);
  const { theme, setTheme, toggleTheme } = useTheme();
  const hydrateTor = useTorStore((state) => state.hydrate);
  const hydrateServices = useServiceStore((state) => state.hydrate);
  const hydrateTunnels = useTunnelStore((state) => state.hydrate);
  const hydrateRuntime = useRuntimeStore((state) => state.hydrate);
  const hydrateUpdates = useUpdateStore((state) => state.hydrate);
  const runtimeStatus = useRuntimeStore((state) => state.status);

  useEffect(() => {
    void hydrateTor();
    void hydrateServices();
    void hydrateTunnels();
    void hydrateRuntime();
    void hydrateUpdates();
  }, [hydrateRuntime, hydrateServices, hydrateTor, hydrateTunnels, hydrateUpdates]);

  if (!setupDismissed && runtimeStatus && !runtimeStatus.ready) {
    return <RuntimeSetup onContinue={() => setSetupDismissed(true)} />;
  }

  return (
    <AppShell
      view={view}
      onViewChange={setView}
      theme={theme}
      onToggleTheme={toggleTheme}
    >
      {view === "services" ? <ServicesView /> : null}
      {view === "tunnels" ? (
        <TunnelsView onOpenServices={() => setView("services")} />
      ) : null}
      {view === "activity" ? <ActivityView /> : null}
      {view === "settings" ? (
        <SettingsView theme={theme} onThemeChange={setTheme} />
      ) : null}
    </AppShell>
  );
}
