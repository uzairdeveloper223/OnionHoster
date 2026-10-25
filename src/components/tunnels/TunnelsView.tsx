import { Network, Plus, Waypoints } from "lucide-react";
import { Button } from "@/components/ui/Button";

const providers = [
  {
    label: "Cloudflare Quick",
    description: "No account. Temporary trycloudflare.com URL.",
    meta: "TESTING",
  },
  {
    label: "ngrok",
    description: "Requires an authtoken. Free-plan limits apply.",
    meta: "TOKEN",
  },
  {
    label: "Custom command",
    description: "Run your own tunnel client with a port template.",
    meta: "ADVANCED",
  },
];

export function TunnelsView() {
  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">Public tunnels</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            0
          </span>
        </div>
        <Button size="sm" disabled title="Tunnel providers are not wired up yet.">
          <Plus className="size-3.5" strokeWidth={1.8} />
          Add tunnel
        </Button>
      </div>

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
        </div>
      </section>

      <div className="mt-4 grid gap-3 sm:grid-cols-3">
        {providers.map((provider) => (
          <div key={provider.label} className="rounded-xl border border-line bg-surface p-4">
            <div className="mb-3 flex items-center justify-between">
              <Waypoints className="size-4 text-muted" strokeWidth={1.6} />
              <span className="font-mono text-[9px] tracking-[0.14em] text-faint">
                {provider.meta}
              </span>
            </div>
            <h3 className="text-[13px] font-medium text-ink">{provider.label}</h3>
            <p className="mt-1 text-xs leading-5 text-muted">{provider.description}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
