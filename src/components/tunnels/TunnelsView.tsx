import { useEffect, useState } from "react";
import {
  CircleAlert,
  Cloud,
  Network,
  Plus,
  ShieldAlert,
  Terminal,
  Waypoints,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { TunnelCard } from "@/components/tunnels/TunnelCard";
import { TunnelDialog } from "@/components/tunnels/TunnelDialog";
import { TunnelLogDialog } from "@/components/tunnels/TunnelLogDialog";
import { isTauri } from "@/lib/app";
import {
  tunnelProviderDescription,
  tunnelProviderLabel,
  type TunnelView,
} from "@/lib/tunnels";
import { useServiceStore } from "@/stores/services";
import { useTunnelStore } from "@/stores/tunnels";

const providerCards = [
  {
    provider: "cloudflare_quick" as const,
    icon: Cloud,
  },
  {
    provider: "cloudflare_named" as const,
    icon: Waypoints,
  },
  {
    provider: "custom" as const,
    icon: Terminal,
  },
];

export function TunnelsView({ onOpenServices }: { onOpenServices: () => void }) {
  const services = useServiceStore((state) => state.services);
  const servicesHydrated = useServiceStore((state) => state.hydrated);
  const hydrateServices = useServiceStore((state) => state.hydrate);
  const refreshServices = useServiceStore((state) => state.refresh);
  const tunnels = useTunnelStore((state) => state.tunnels);
  const hydrated = useTunnelStore((state) => state.hydrated);
  const busy = useTunnelStore((state) => state.busy);
  const lastError = useTunnelStore((state) => state.lastError);
  const clearError = useTunnelStore((state) => state.clearError);
  const hydrateTunnels = useTunnelStore((state) => state.hydrate);
  const refreshTunnels = useTunnelStore((state) => state.refresh);
  const start = useTunnelStore((state) => state.start);
  const stop = useTunnelStore((state) => state.stop);
  const restart = useTunnelStore((state) => state.restart);
  const remove = useTunnelStore((state) => state.remove);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<TunnelView | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<TunnelView | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [logTarget, setLogTarget] = useState<TunnelView | null>(null);

  useEffect(() => {
    void hydrateServices();
    void hydrateTunnels();

    if (!isTauri) {
      return;
    }

    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        void refreshServices();
        void refreshTunnels();
      }
    }, 5000);

    return () => window.clearInterval(timer);
  }, [hydrateServices, hydrateTunnels, refreshServices, refreshTunnels]);

  async function confirmDelete() {
    if (!deleteTarget) {
      return;
    }

    setDeleting(true);
    try {
      await remove(deleteTarget.id);
      setDeleteTarget(null);
    } catch {
      // The store keeps the error for the banner.
    } finally {
      setDeleting(false);
    }
  }

  function run(action: () => Promise<unknown>) {
    void action().catch(() => {
      // The store keeps the error for the banner.
    });
  }

  function openCreate() {
    setEditing(null);
    setDialogOpen(true);
  }

  function openEdit(tunnel: TunnelView) {
    setEditing(tunnel);
    setDialogOpen(true);
  }

  const running = tunnels.filter((tunnel) => tunnel.state === "running").length;

  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-4 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">Public tunnels</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            {tunnels.length}
          </span>
          {running > 0 ? (
            <span className="font-mono text-[10px] text-faint">{running} live</span>
          ) : null}
        </div>
        <Button
          size="sm"
          onClick={openCreate}
          disabled={!hydrated || !servicesHydrated || services.length === 0}
          title={
            servicesHydrated && services.length === 0
              ? "Create and start a service first."
              : undefined
          }
        >
          <Plus className="size-3.5" strokeWidth={1.8} />
          Add tunnel
        </Button>
      </div>

      <div className="mb-4 flex items-start gap-2.5 rounded-lg border border-line bg-surface px-3 py-2.5">
        <ShieldAlert
          className="mt-0.5 size-4 shrink-0 text-ink"
          strokeWidth={1.6}
        />
        <p className="text-[11px] leading-4 text-muted">
          Tunnels expose a service on the public internet, outside Tor. They are
          optional and are not anonymous. Tor onion hosting keeps working while a
          tunnel is active.
        </p>
      </div>

      {lastError ? (
        <div className="mb-4 flex items-start gap-2 rounded-lg border border-line bg-surface px-3 py-2.5">
          <CircleAlert className="mt-0.5 size-3.5 shrink-0 text-ink" strokeWidth={1.6} />
          <p className="min-w-0 flex-1 break-words text-[11px] leading-4 text-muted">
            {lastError}
          </p>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={clearError}
            aria-label="Dismiss error"
          >
            <X className="size-3.5" strokeWidth={1.6} />
          </Button>
        </div>
      ) : null}

      {!hydrated || !servicesHydrated ? (
        <div className="grid gap-3 sm:grid-cols-2">
          {[0, 1].map((index) => (
            <div
              key={index}
              className="h-64 animate-pulse rounded-xl border border-line bg-surface"
            />
          ))}
        </div>
      ) : tunnels.length === 0 ? (
        <>
          <section className="relative overflow-hidden rounded-2xl border border-line bg-surface">
            <div className="onion-grid pointer-events-none absolute inset-0 opacity-30" />
            <div className="relative flex flex-col items-center px-6 py-16 text-center">
              <div className="mb-5 flex size-14 items-center justify-center rounded-2xl border border-line bg-bg">
                <Network className="size-7 text-muted" strokeWidth={1.4} />
              </div>
              <h2 className="text-lg font-medium tracking-tight text-ink">No tunnels</h2>
              <p className="mt-2 max-w-md text-sm leading-6 text-muted">
                Tor onion hosting works without a tunnel. Add one when you also need a
                clearnet URL.
              </p>
              {services.length === 0 ? (
                <>
                  <Button className="mt-6" onClick={onOpenServices}>
                    <Plus className="size-4" strokeWidth={1.8} />
                    Create a service
                  </Button>
                  <p className="mt-3 font-mono text-[11px] text-faint">
                    A tunnel needs a running service. Create one first.
                  </p>
                </>
              ) : (
                <Button className="mt-6" onClick={openCreate}>
                  <Plus className="size-4" strokeWidth={1.8} />
                  Add a tunnel
                </Button>
              )}
            </div>
          </section>

          <div className="mt-4 grid gap-3 sm:grid-cols-3">
            {providerCards.map(({ provider, icon: Icon }) => (
              <div
                key={provider}
                className="rounded-xl border border-line bg-surface p-4"
              >
                <div className="mb-3 flex items-center justify-between">
                  <Icon className="size-4 text-muted" strokeWidth={1.6} />
                  <span className="font-mono text-[9px] tracking-[0.14em] text-faint">
                    {provider === "cloudflare_quick"
                      ? "TEMP"
                      : provider === "cloudflare_named"
                        ? "TOKEN"
                        : "ADVANCED"}
                  </span>
                </div>
                <h3 className="text-[13px] font-medium text-ink">
                  {tunnelProviderLabel(provider)}
                </h3>
                <p className="mt-1 text-xs leading-5 text-muted">
                  {tunnelProviderDescription(provider)}
                </p>
              </div>
            ))}
          </div>
        </>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {tunnels.map((tunnel) => (
            <TunnelCard
              key={tunnel.id}
              tunnel={tunnel}
              service={services.find((service) => service.id === tunnel.serviceId)}
              busy={Boolean(busy[tunnel.id])}
              onStart={() => run(() => start(tunnel.id))}
              onStop={() => run(() => stop(tunnel.id))}
              onRestart={() => run(() => restart(tunnel.id))}
              onEdit={() => openEdit(tunnel)}
              onDelete={() => setDeleteTarget(tunnel)}
              onLogs={() => setLogTarget(tunnel)}
            />
          ))}
        </div>
      )}

      <p className="mt-6 text-[11px] leading-5 text-faint">
        Cloudflare Quick URLs are temporary. Named tunnels require a hostname on a
        domain you control and a matching ingress rule in Cloudflare. Tokens are
        stored locally with owner-only permissions.
      </p>

      <TunnelDialog
        open={dialogOpen}
        editing={editing}
        onClose={() => {
          setDialogOpen(false);
          setEditing(null);
        }}
      />

      <TunnelLogDialog tunnel={logTarget} onClose={() => setLogTarget(null)} />

      {deleteTarget ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-[2px]"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !deleting) {
              setDeleteTarget(null);
            }
          }}
        >
          <div
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="delete-tunnel-title"
            className="panel w-full max-w-sm rounded-2xl p-5 shadow-2xl"
          >
            <h2 id="delete-tunnel-title" className="text-sm font-medium text-ink">
              Delete “{deleteTarget.name}”?
            </h2>
            <p className="mt-2 text-xs leading-5 text-muted">
              The tunnel will stop and its saved provider settings will be removed.
              This cannot be undone.
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <Button
                variant="ghost"
                disabled={deleting}
                onClick={() => setDeleteTarget(null)}
              >
                Cancel
              </Button>
              <Button disabled={deleting} onClick={() => void confirmDelete()}>
                Delete tunnel
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
