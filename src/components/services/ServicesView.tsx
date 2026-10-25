import { Cable, FolderOpen, Lock, Plus, Server } from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";

const serviceTypes = [
  {
    icon: FolderOpen,
    label: "Static site",
    description: "Serve a folder or a single HTML file.",
  },
  {
    icon: Server,
    label: "Local server",
    description: "Point to a running HTTP or HTTPS port.",
  },
  {
    icon: Cable,
    label: "TCP service",
    description: "Publish a raw TCP port through Tor.",
  },
];

export function ServicesView() {
  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">All services</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            0
          </span>
        </div>
        <Button
          size="sm"
          disabled
          title="Service creation is the next implementation step."
        >
          <Plus className="size-3.5" strokeWidth={1.8} />
          New service
        </Button>
      </div>

      <section className="relative overflow-hidden rounded-2xl border border-line bg-surface">
        <div className="onion-grid pointer-events-none absolute inset-0 opacity-40" />
        <div className="relative flex flex-col items-center px-6 py-20 text-center">
          <div className="mb-5 flex size-16 items-center justify-center rounded-2xl border border-line bg-bg">
            <OnionMark className="size-8" />
          </div>
          <h2 className="text-lg font-medium tracking-tight text-ink">No services yet</h2>
          <p className="mt-2 max-w-md text-sm leading-6 text-muted">
            Publish a static folder, a running local server, or a TCP port through Tor.
          </p>
          <Button
            className="mt-6"
            disabled
            title="Service creation is the next implementation step."
          >
            <Plus className="size-4" strokeWidth={1.8} />
            Create a service
          </Button>
          <p className="mt-3 font-mono text-[11px] text-faint">
            Service runtime arrives in the next step.
          </p>
        </div>
      </section>

      <div className="mt-4 grid gap-3 sm:grid-cols-3">
        {serviceTypes.map((type) => {
          const Icon = type.icon;

          return (
            <div key={type.label} className="rounded-xl border border-line bg-surface p-4">
              <div className="mb-3 flex size-8 items-center justify-center rounded-lg border border-line bg-bg">
                <Icon className="size-4 text-muted" strokeWidth={1.6} />
              </div>
              <h3 className="text-[13px] font-medium text-ink">{type.label}</h3>
              <p className="mt-1 text-xs leading-5 text-muted">{type.description}</p>
            </div>
          );
        })}
      </div>

      <p className="mt-6 flex items-center gap-2 text-[11px] text-faint">
        <Lock className="size-3.5" strokeWidth={1.6} />
        Services bind to localhost only.
      </p>
    </div>
  );
}
