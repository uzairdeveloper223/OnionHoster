import { useState } from "react";
import {
  Cable,
  Check,
  CircleAlert,
  Copy,
  FileText,
  FolderOpen,
  Play,
  RotateCw,
  Server,
  Square,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/StatusDot";
import { copyText } from "@/lib/clipboard";
import {
  formatBytes,
  serviceAddress,
  serviceStateLabel,
  serviceSummary,
  serviceTone,
  serviceTypeLabel,
  type ServiceView,
} from "@/lib/services";
import { cn } from "@/lib/utils";

function SourceIcon({ service }: { service: ServiceView }) {
  const Icon =
    service.source.kind === "static"
      ? FolderOpen
      : service.source.kind === "tcp"
        ? Cable
        : service.source.kind === "default_page"
          ? FileText
          : Server;

  return <Icon className="size-4 text-muted" strokeWidth={1.6} />;
}

export function ServiceCard({
  service,
  busy,
  onStart,
  onStop,
  onRestart,
  onDelete,
}: {
  service: ServiceView;
  busy: boolean;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onDelete: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const address = serviceAddress(service);
  const running = service.state === "running";

  async function handleCopy() {
    if (!address) {
      return;
    }

    if (await copyText(address)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    }
  }

  return (
    <article className="flex flex-col rounded-xl border border-line bg-surface p-4">
      <header className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <div className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-bg">
            <SourceIcon service={service} />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-[13px] font-medium text-ink">{service.name}</h3>
            <p className="mt-0.5 truncate font-mono text-[10px] text-faint">
              {serviceTypeLabel(service.source)}
            </p>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1.5 rounded-full border border-line px-2 py-1 text-[10px] text-muted">
          <StatusDot status={serviceTone(service.state)} />
          {serviceStateLabel(service)}
        </div>
      </header>

      <div className="mt-3 min-w-0">
        {address ? (
          <div className="flex items-center gap-1.5 rounded-lg border border-line bg-bg px-2.5 py-2">
            <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-ink">
              {address}
            </code>
            <Button
              variant="ghost"
              size="icon"
              className="size-6"
              onClick={() => void handleCopy()}
              aria-label={copied ? "Address copied" : "Copy onion address"}
              title={copied ? "Copied" : "Copy address"}
            >
              {copied ? (
                <Check className="size-3.5" strokeWidth={1.8} />
              ) : (
                <Copy className="size-3.5" strokeWidth={1.6} />
              )}
            </Button>
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-line px-2.5 py-2 font-mono text-[11px] text-faint">
            No onion address yet
          </div>
        )}
      </div>

      <p
        className="mt-2 truncate font-mono text-[10px] text-faint"
        title={serviceSummary(service.source)}
      >
        {serviceSummary(service.source)}
      </p>

      {service.error ? (
        <div className="mt-2 flex items-start gap-2 rounded-lg border border-line bg-bg px-2.5 py-2">
          <CircleAlert className="mt-0.5 size-3.5 shrink-0 text-ink" strokeWidth={1.6} />
          <p className="min-w-0 break-words text-[11px] leading-4 text-muted">
            {service.error}
          </p>
        </div>
      ) : null}

      {service.state === "waiting_for_tor" ? (
        <p className="mt-2 text-[11px] leading-4 text-faint">
          Start Tor to publish this service. It will continue automatically when the
          runtime is ready.
        </p>
      ) : null}

      <div className="mt-3 grid grid-cols-3 gap-2 border-t border-line pt-3">
        <div>
          <div className="font-mono text-[10px] text-faint">Requests</div>
          <div className="mt-0.5 font-mono text-[11px] text-ink">
            {service.stats.requests.toLocaleString()}
          </div>
        </div>
        <div>
          <div className="font-mono text-[10px] text-faint">In</div>
          <div className="mt-0.5 font-mono text-[11px] text-ink">
            {formatBytes(service.stats.bytesIn)}
          </div>
        </div>
        <div>
          <div className="font-mono text-[10px] text-faint">Out</div>
          <div className="mt-0.5 font-mono text-[11px] text-ink">
            {formatBytes(service.stats.bytesOut)}
          </div>
        </div>
      </div>

      <footer className="mt-3 flex items-center gap-1.5">
        <Button
          size="sm"
          variant={running ? "secondary" : "primary"}
          className="flex-1"
          disabled={busy}
          onClick={running ? onStop : onStart}
        >
          {running ? (
            <Square className="size-3" strokeWidth={1.8} />
          ) : (
            <Play className="size-3" strokeWidth={1.8} />
          )}
          {running ? "Stop" : "Start"}
        </Button>
        <Button
          size="icon"
          variant="ghost"
          disabled={busy || !running}
          onClick={onRestart}
          aria-label="Restart service"
          title="Restart"
        >
          <RotateCw className={cn("size-3.5", busy && running && "animate-spin")} strokeWidth={1.6} />
        </Button>
        <Button
          size="icon"
          variant="ghost"
          disabled={busy}
          onClick={onDelete}
          aria-label="Delete service"
          title="Delete"
        >
          <Trash2 className="size-3.5" strokeWidth={1.6} />
        </Button>
      </footer>
    </article>
  );
}
