import { useEffect, useMemo, useRef } from "react";
import { Eraser, Terminal } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/StatusDot";
import { formatLogTime, torLabel, torTone } from "@/lib/tor";
import { cn } from "@/lib/utils";
import { useServiceStore } from "@/stores/services";
import { useTorStore } from "@/stores/tor";

const levelClass: Record<string, string> = {
  error: "font-semibold text-ink",
  warn: "text-ink",
  info: "text-faint",
};

interface CombinedLog {
  key: string;
  source: "tor" | "service";
  sourceName: string | null;
  level: string;
  message: string;
  timestampMs: number;
}

export function ActivityView() {
  const torLogs = useTorStore((state) => state.logs);
  const status = useTorStore((state) => state.status);
  const clearTorLogs = useTorStore((state) => state.clearLogs);
  const serviceLogs = useServiceStore((state) => state.logs);
  const services = useServiceStore((state) => state.services);
  const clearServiceLogs = useServiceStore((state) => state.clearLogs);
  const scrollRef = useRef<HTMLDivElement>(null);

  const logs = useMemo(() => {
    const names = new Map(services.map((service) => [service.id, service.name]));

    const combined: CombinedLog[] = [
      ...torLogs.map((log) => ({
        key: `tor-${log.id}`,
        source: "tor" as const,
        sourceName: null,
        level: log.level,
        message: log.message,
        timestampMs: log.timestampMs,
      })),
      ...serviceLogs.map((log, index) => ({
        key: `service-${log.id}-${log.timestampMs}-${index}`,
        source: "service" as const,
        sourceName: names.get(log.id) ?? log.id,
        level: log.level,
        message: log.message,
        timestampMs: log.timestampMs,
      })),
    ];

    return combined.sort((left, right) => left.timestampMs - right.timestampMs);
  }, [serviceLogs, services, torLogs]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [logs.length]);

  function clearLogs() {
    clearTorLogs();
    clearServiceLogs();
  }

  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">Event stream</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            {logs.length} {logs.length === 1 ? "event" : "events"}
          </span>
        </div>
        <div className="flex items-center gap-2 text-[11px] text-faint">
          <StatusDot status={torTone(status)} />
          {torLabel(status)}
        </div>
      </div>

      <section className="overflow-hidden rounded-xl border border-line bg-surface">
        <header className="flex h-10 items-center justify-between border-b border-line px-4">
          <div className="flex items-center gap-2 font-mono text-[11px] text-faint">
            <Terminal className="size-3.5" strokeWidth={1.6} />
            live.log
          </div>
          {logs.length > 0 ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-1.5 text-[10px]"
              onClick={clearLogs}
            >
              <Eraser className="size-3" strokeWidth={1.6} />
              Clear view
            </Button>
          ) : (
            <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              Idle
            </span>
          )}
        </header>

        {logs.length === 0 ? (
          <div className="flex min-h-[320px] flex-col items-center justify-center px-6 py-12 text-center">
            <div className="mb-4 flex size-12 items-center justify-center rounded-xl border border-line bg-bg">
              <Terminal className="size-6 text-muted" strokeWidth={1.4} />
            </div>
            <h2 className="text-sm font-medium text-ink">No activity yet</h2>
            <p className="mt-1.5 max-w-sm text-xs leading-5 text-muted">
              Tor bootstrap progress, descriptor updates, service events, and tunnel
              output will appear here.
            </p>
          </div>
        ) : (
          <div
            ref={scrollRef}
            className="max-h-[430px] min-h-[320px] overflow-y-auto"
            aria-live="polite"
          >
            {logs.map((log) => (
              <div
                key={log.key}
                className="grid grid-cols-[4.25rem_3rem_minmax(0,1fr)] gap-3 border-b border-line/70 px-4 py-2 font-mono text-[11px] last:border-b-0"
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
                  {log.sourceName ? (
                    <span className="mr-2 inline-block max-w-[10rem] truncate rounded border border-line px-1 py-0.5 align-baseline text-[9px] uppercase tracking-wide text-faint">
                      {log.sourceName}
                    </span>
                  ) : null}
                  {log.message}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      <p className="mt-4 text-[11px] leading-5 text-faint">
        Private keys, bridge lines, and tunnel tokens will be removed from the event
        stream.
      </p>
    </div>
  );
}
