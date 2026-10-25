import { useEffect, useState } from "react";
import {
  CircleAlert,
  FileText,
  FolderOpen,
  Lock,
  Plus,
  Server,
  X,
} from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { CreateServiceDialog } from "@/components/services/CreateServiceDialog";
import { ServiceCard } from "@/components/services/ServiceCard";
import { Button } from "@/components/ui/Button";
import { isTauri } from "@/lib/app";
import type { ServiceView } from "@/lib/services";
import { useServiceStore } from "@/stores/services";

const serviceTypes = [
  {
    icon: FolderOpen,
    label: "Static site",
    description: "Serve a folder or a single HTML file.",
  },
  {
    icon: Server,
    label: "Local server",
    description: "Proxy a running HTTP or HTTPS port.",
  },
  {
    icon: Lock,
    label: "TCP service",
    description: "Publish a raw TCP port through Tor.",
  },
  {
    icon: FileText,
    label: "OnionHoster page",
    description: "Serve a built-in page about the project.",
  },
];

export function ServicesView() {
  const services = useServiceStore((state) => state.services);
  const hydrated = useServiceStore((state) => state.hydrated);
  const busy = useServiceStore((state) => state.busy);
  const lastError = useServiceStore((state) => state.lastError);
  const clearError = useServiceStore((state) => state.clearError);
  const hydrate = useServiceStore((state) => state.hydrate);
  const refresh = useServiceStore((state) => state.refresh);
  const start = useServiceStore((state) => state.start);
  const stop = useServiceStore((state) => state.stop);
  const restart = useServiceStore((state) => state.restart);
  const remove = useServiceStore((state) => state.remove);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ServiceView | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    void hydrate();

    if (!isTauri) {
      return;
    }

    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        void refresh();
      }
    }, 5000);

    return () => window.clearInterval(timer);
  }, [hydrate, refresh]);

  const running = services.filter((service) => service.state === "running").length;

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

  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">All services</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            {services.length}
          </span>
          {running > 0 ? (
            <span className="font-mono text-[10px] text-faint">{running} published</span>
          ) : null}
        </div>
        <Button size="sm" onClick={() => setDialogOpen(true)}>
          <Plus className="size-3.5" strokeWidth={1.8} />
          New service
        </Button>
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

      {!hydrated ? (
        <div className="grid gap-3 sm:grid-cols-2">
          {[0, 1].map((index) => (
            <div
              key={index}
              className="h-56 animate-pulse rounded-xl border border-line bg-surface"
            />
          ))}
        </div>
      ) : services.length === 0 ? (
        <>
          <section className="relative overflow-hidden rounded-2xl border border-line bg-surface">
            <div className="onion-grid pointer-events-none absolute inset-0 opacity-40" />
            <div className="relative flex flex-col items-center px-6 py-20 text-center">
              <div className="mb-5 flex size-16 items-center justify-center rounded-2xl border border-line bg-bg">
                <OnionMark className="size-8" />
              </div>
              <h2 className="text-lg font-medium tracking-tight text-ink">
                No services yet
              </h2>
              <p className="mt-2 max-w-lg text-sm leading-6 text-balance text-muted">
                Publish a static folder, a running local server, a TCP port, or the
                built-in OnionHoster page through Tor.
              </p>
              <Button className="mt-6" onClick={() => setDialogOpen(true)}>
                <Plus className="size-4" strokeWidth={1.8} />
                Create a service
              </Button>
              <p className="mt-3 font-mono text-[11px] text-faint">
                {isTauri
                  ? "Keys are stored locally and addresses survive restarts."
                  : "The service runtime is only available in the desktop app."}
              </p>
            </div>
          </section>

          <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {serviceTypes.map((type) => {
              const Icon = type.icon;

              return (
                <div
                  key={type.label}
                  className="rounded-xl border border-line bg-surface p-4"
                >
                  <div className="mb-3 flex size-8 items-center justify-center rounded-lg border border-line bg-bg">
                    <Icon className="size-4 text-muted" strokeWidth={1.6} />
                  </div>
                  <h3 className="text-[13px] font-medium text-ink">{type.label}</h3>
                  <p className="mt-1 text-xs leading-5 text-muted">{type.description}</p>
                </div>
              );
            })}
          </div>
        </>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {services.map((service) => (
            <ServiceCard
              key={service.id}
              service={service}
              busy={Boolean(busy[service.id])}
              onStart={() => run(() => start(service.id))}
              onStop={() => run(() => stop(service.id))}
              onRestart={() => run(() => restart(service.id))}
              onDelete={() => setDeleteTarget(service)}
            />
          ))}
        </div>
      )}

      <p className="mt-6 flex items-center gap-2 text-[11px] text-faint">
        <Lock className="size-3.5" strokeWidth={1.6} />
        Local servers and published onion ports bind to localhost only.
      </p>

      <CreateServiceDialog open={dialogOpen} onClose={() => setDialogOpen(false)} />

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
            aria-labelledby="delete-service-title"
            className="panel w-full max-w-sm rounded-2xl p-5 shadow-2xl"
          >
            <h2 id="delete-service-title" className="text-sm font-medium text-ink">
              Delete “{deleteTarget.name}”?
            </h2>
            <p className="mt-2 text-xs leading-5 text-muted">
              The service will stop and its saved onion address will be removed. This
              cannot be undone.
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
                Delete service
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
