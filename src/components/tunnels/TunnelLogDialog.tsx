import { useEffect, useRef } from "react";
import { Eraser, Terminal, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { formatLogTime } from "@/lib/tor";
import type { TunnelView } from "@/lib/tunnels";
import { useTunnelStore } from "@/stores/tunnels";
import { cn } from "@/lib/utils";

const levelClass: Record<string, string> = {
  error: "font-semibold text-ink",
  warn: "text-ink",
  info: "text-faint",
};

export function TunnelLogDialog({
  tunnel,
  onClose,
}: {
  tunnel: TunnelView | null;
  onClose: () => void;
}) {
  const logs = useTunnelStore((state) =>
    tunnel ? state.logs[tunnel.id] ?? [] : [],
  );
  const loadLogs = useTunnelStore((state) => state.loadLogs);
  const clearLogs = useTunnelStore((state) => state.clearLogs);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (tunnel) {
      void loadLogs(tunnel.id);
    }
  }, [loadLogs, tunnel]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [logs.length]);

  useEffect(() => {
    if (!tunnel) {
      return;
    }
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose, tunnel]);

  if (!tunnel) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-[2px]"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="tunnel-log-title"
        className="panel flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl shadow-2xl"
      >
        <header className="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
          <div className="min-w-0">
            <h2 id="tunnel-log-title" className="truncate text-sm font-medium text-ink">
              {tunnel.name}
            </h2>
            <p className="mt-0.5 font-mono text-[10px] text-faint">
              tunnel.log / {tunnel.id.slice(0, 8)}
            </p>
          </div>
          <div className="flex items-center gap-1.5">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[10px]"
              onClick={() => clearLogs(tunnel.id)}
            >
              <Eraser className="size-3" strokeWidth={1.6} />
              Clear
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={onClose}
              aria-label="Close logs"
            >
              <X className="size-4" strokeWidth={1.6} />
            </Button>
          </div>
        </header>

        {logs.length === 0 ? (
          <div className="flex min-h-[280px] flex-col items-center justify-center px-6 py-12 text-center">
            <div className="mb-4 flex size-12 items-center justify-center rounded-xl border border-line bg-bg">
              <Terminal className="size-6 text-muted" strokeWidth={1.4} />
            </div>
            <h3 className="text-sm font-medium text-ink">No tunnel output yet</h3>
            <p className="mt-1.5 max-w-sm text-xs leading-5 text-muted">
              Start the tunnel to see the client output. Tokens are redacted.
            </p>
          </div>
        ) : (
          <div
            ref={scrollRef}
            className="max-h-[430px] min-h-[280px] overflow-y-auto"
            aria-live="polite"
          >
            {logs.map((log) => (
              <div
                key={`${log.id}-${log.timestampMs}`}
                className="grid grid-cols-[4.25rem_2.75rem_minmax(0,1fr)] gap-3 border-b border-line/70 px-4 py-2 font-mono text-[11px] last:border-b-0"
              >
                <span className="text-faint">{formatLogTime(log.timestampMs)}</span>
                <span
                  className={cn(
                    "uppercase tracking-wide",
                    levelClass[log.level] ?? "text-faint",
                  )}
                >
                  {log.level}
                </span>
                <span className="min-w-0 whitespace-pre-wrap break-words text-muted">
                  {log.message}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
