import { useState } from "react";
import {
  Cable,
  Check,
  CircleAlert,
  Copy,
  Download,
  FileText,
  FolderOpen,
  FolderUp,
  KeyRound,
  MessagesSquare,
  Play,
  RotateCw,
  Server,
  ShieldCheck,
  Square,
  Trash2,
  X,
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
  type ClientAuth,
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
          : service.source.kind === "chat"
            ? MessagesSquare
            : service.source.kind === "file_share"
              ? FolderUp
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
  const [showKeys, setShowKeys] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const address = serviceAddress(service);
  const running = service.state === "running";
  const clientAuth = service.onion.clientAuth ?? [];

  function authLine(auth: ClientAuth): string | null {
    return service.onion.serviceId
      ? `${service.onion.serviceId}:descriptor:x25519:${auth.privateKey}`
      : null;
  }

  async function handleCopy() {
    if (!address) {
      return;
    }

    if (await copyText(address)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    }
  }

  async function copyClientKey(auth: ClientAuth) {
    const line = authLine(auth);
    if (!line) {
      return;
    }
    if (await copyText(line)) {
      setCopiedKey(auth.name);
      window.setTimeout(() => setCopiedKey(null), 1600);
    }
  }

  function downloadClientKey(auth: ClientAuth) {
    const line = authLine(auth);
    if (!line || !service.onion.serviceId) {
      return;
    }
    const blob = new Blob([`${line}\n`], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${service.onion.serviceId}.auth_private`;
    anchor.click();
    URL.revokeObjectURL(url);
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
            <div className="mt-1 flex flex-wrap items-center gap-1.5">
              <span className="truncate font-mono text-[10px] text-faint">
                {serviceTypeLabel(service.source)}
              </span>
              {service.onion.private ? (
                <span className="inline-flex items-center gap-1 rounded-full border border-line px-1.5 py-0.5 font-mono text-[9px] text-muted">
                  <ShieldCheck className="size-2.5" strokeWidth={1.8} />
                  Private
                </span>
              ) : null}
              {service.onion.pow ? (
                <span className="rounded-full border border-line px-1.5 py-0.5 font-mono text-[9px] text-muted">
                  PoW
                </span>
              ) : null}
              {service.onion.maxStreams ? (
                <span className="rounded-full border border-line px-1.5 py-0.5 font-mono text-[9px] text-muted">
                  {service.onion.maxStreams} streams
                </span>
              ) : null}
            </div>
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
        {service.onion.private ? (
          <Button
            size="icon"
            variant="ghost"
            onClick={() => setShowKeys(true)}
            aria-label="Client authorization key"
            title="Client key"
          >
            <KeyRound className="size-3.5" strokeWidth={1.6} />
          </Button>
        ) : null}
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

      {showKeys ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-4 backdrop-blur-sm"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              setShowKeys(false);
            }
          }}
        >
          <div className="w-full max-w-lg overflow-hidden rounded-xl border border-line bg-surface shadow-2xl">
            <header className="flex items-start justify-between gap-3 border-b border-line px-4 py-3">
              <div className="min-w-0">
                <h3 className="text-[13px] font-medium text-ink">Client authorization</h3>
                <p className="mt-0.5 text-[11px] leading-4 text-muted">
                  Add this key to Tor Browser before opening{" "}
                  {service.onion.serviceId ? "the onion address" : "the address"}.
                </p>
              </div>
              <Button
                size="icon"
                variant="ghost"
                onClick={() => setShowKeys(false)}
                aria-label="Close client key dialog"
              >
                <X className="size-4" strokeWidth={1.7} />
              </Button>
            </header>
            <div className="max-h-[58vh] space-y-3 overflow-y-auto p-4">
              {clientAuth.length === 0 ? (
                <p className="rounded-lg border border-line bg-bg px-3 py-2.5 text-[11px] leading-4 text-muted">
                  No client keys exist for this service yet.
                </p>
              ) : null}
              {clientAuth.map((auth) => {
                const line = authLine(auth);
                return (
                  <div
                    key={auth.publicKey}
                    className="rounded-lg border border-line bg-bg p-3"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <span className="truncate text-[12px] font-medium text-ink">
                        {auth.name}
                      </span>
                      <span className="shrink-0 font-mono text-[9px] text-faint">
                        {new Date(auth.createdAt).toLocaleDateString()}
                      </span>
                    </div>
                    <code className="mt-2 block break-all rounded-md border border-line bg-surface px-2.5 py-2 font-mono text-[10px] leading-4 text-ink">
                      {line ?? auth.privateKey}
                    </code>
                    <div className="mt-2 flex flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={!line}
                        onClick={() => void copyClientKey(auth)}
                      >
                        {copiedKey === auth.name ? (
                          <Check className="size-3.5" strokeWidth={1.8} />
                        ) : (
                          <Copy className="size-3.5" strokeWidth={1.6} />
                        )}
                        {copiedKey === auth.name ? "Copied" : "Copy key"}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={!line}
                        onClick={() => downloadClientKey(auth)}
                      >
                        <Download className="size-3.5" strokeWidth={1.6} />
                        .auth_private
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
            <footer className="flex justify-end border-t border-line px-4 py-3">
              <Button size="sm" variant="secondary" onClick={() => setShowKeys(false)}>
                Close
              </Button>
            </footer>
          </div>
        </div>
      ) : null}
    </article>
  );
}
