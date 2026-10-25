import { useState } from "react";
import {
  Check,
  Cloud,
  Copy,
  Loader2,
  Pencil,
  Play,
  RotateCw,
  ScrollText,
  Square,
  Terminal,
  Trash2,
  TriangleAlert,
  Waypoints,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/StatusDot";
import { copyText } from "@/lib/clipboard";
import { serviceStateLabel, type ServiceView } from "@/lib/services";
import {
  tunnelProviderLabel,
  tunnelStateLabel,
  tunnelTone,
  type TunnelView,
} from "@/lib/tunnels";
import { cn } from "@/lib/utils";

function ProviderIcon({ provider }: { provider: TunnelView["provider"] }) {
  const Icon =
    provider === "cloudflare_named"
      ? Waypoints
      : provider === "custom"
        ? Terminal
        : Cloud;

  return <Icon className="size-4 text-muted" strokeWidth={1.6} />;
}

export function TunnelCard({
  tunnel,
  service,
  busy,
  onStart,
  onStop,
  onRestart,
  onEdit,
  onDelete,
  onLogs,
}: {
  tunnel: TunnelView;
  service: ServiceView | undefined;
  busy: boolean;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onLogs: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const running = tunnel.state === "running";
  const starting = tunnel.state === "starting";
  const serviceStopped = !service || service.state !== "running";

  async function copyUrl() {
    if (!tunnel.publicUrl) {
      return;
    }
    if (await copyText(tunnel.publicUrl)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    }
  }

  return (
    <article className="flex flex-col rounded-xl border border-line bg-surface p-4">
      <header className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <div className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-bg">
            <ProviderIcon provider={tunnel.provider} />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-[13px] font-medium text-ink">{tunnel.name}</h3>
            <p className="mt-0.5 truncate font-mono text-[10px] text-faint">
              {tunnelProviderLabel(tunnel.provider)}
            </p>
          </div>
        </div>
        <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-line px-2 py-1 font-mono text-[10px] text-faint">
          <StatusDot
            status={tunnelTone(tunnel.state)}
            className={starting ? "animate-pulse" : undefined}
          />
          {tunnelStateLabel(tunnel.state)}
        </span>
      </header>

      <div className="mt-4 space-y-2">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-[11px] text-faint">Service</span>
          <span className="truncate text-[11px] text-muted">
            {service ? (
              <>
                {service.name}
                <span className="ml-1.5 font-mono text-[10px] text-faint">
                  {serviceStateLabel(service)}
                </span>
              </>
            ) : (
              "Missing service"
            )}
          </span>
        </div>
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-[11px] text-faint">Local target</span>
          <span className="truncate font-mono text-[10px] text-muted">
            {tunnel.localTarget ?? "Starts with the tunnel"}
          </span>
        </div>
      </div>

      {tunnel.publicUrl ? (
        <div className="mt-3 flex items-center gap-1.5 rounded-lg border border-line bg-bg px-2.5 py-2">
          <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-ink">
            {tunnel.publicUrl}
          </code>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={() => void copyUrl()}
            aria-label="Copy public URL"
          >
            {copied ? (
              <Check className="size-3.5" strokeWidth={1.8} />
            ) : (
              <Copy className="size-3.5" strokeWidth={1.6} />
            )}
          </Button>
        </div>
      ) : (
        <div className="mt-3 rounded-lg border border-dashed border-line bg-bg px-2.5 py-2 text-[11px] text-faint">
          {starting
            ? "Waiting for the tunnel client to report a public URL."
            : "No public URL yet."}
        </div>
      )}

      {tunnel.error ? (
        <p className="mt-3 rounded-lg border border-line bg-bg px-2.5 py-2 text-[11px] leading-4 text-muted">
          {tunnel.error}
        </p>
      ) : null}

      {serviceStopped ? (
        <p className="mt-3 flex items-start gap-2 rounded-lg border border-line bg-bg px-2.5 py-2 text-[11px] leading-4 text-muted">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" strokeWidth={1.6} />
          Start the service first. A tunnel cannot reach a stopped service.
        </p>
      ) : null}

      <footer className="mt-auto flex items-center justify-between gap-2 pt-4">
        <div className="flex items-center gap-1.5">
          {running || starting ? (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={onStop}
            >
              {starting ? (
                <Loader2 className="size-3 animate-spin" strokeWidth={1.8} />
              ) : (
                <Square className="size-3" strokeWidth={1.8} />
              )}
              Stop
            </Button>
          ) : (
            <Button
              size="sm"
              variant={tunnel.state === "error" ? "primary" : "secondary"}
              disabled={busy || serviceStopped}
              onClick={onStart}
              title={serviceStopped ? "Start the service first." : undefined}
            >
              <Play className="size-3" strokeWidth={1.8} />
              {tunnel.state === "error" ? "Retry" : "Start"}
            </Button>
          )}
          <Button
            size="icon"
            variant="ghost"
            className={cn("size-8", !running && "opacity-40")}
            disabled={busy || !running}
            onClick={onRestart}
            aria-label="Restart tunnel"
            title="Restart tunnel"
          >
            <RotateCw className="size-3.5" strokeWidth={1.6} />
          </Button>
          <Button
            size="icon"
            variant="ghost"
            className="size-8"
            onClick={onLogs}
            aria-label="Tunnel logs"
            title="Tunnel logs"
          >
            <ScrollText className="size-3.5" strokeWidth={1.6} />
          </Button>
        </div>

        <div className="flex items-center gap-1.5">
          <Button
            size="icon"
            variant="ghost"
            className="size-8"
            disabled={busy || running || starting}
            onClick={onEdit}
            aria-label="Edit tunnel"
            title={running || starting ? "Stop the tunnel before editing." : "Edit tunnel"}
          >
            <Pencil className="size-3.5" strokeWidth={1.6} />
          </Button>
          <Button
            size="icon"
            variant="ghost"
            className="size-8"
            disabled={busy}
            onClick={onDelete}
            aria-label="Delete tunnel"
            title="Delete tunnel"
          >
            <Trash2 className="size-3.5" strokeWidth={1.6} />
          </Button>
        </div>
      </footer>
    </article>
  );
}
