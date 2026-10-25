import { Terminal } from "lucide-react";
import { StatusDot } from "@/components/ui/StatusDot";

export function ActivityView() {
  return (
    <div className="mx-auto w-full max-w-5xl px-6 py-7">
      <div className="mb-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-ink">Event stream</h2>
          <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] text-faint">
            0 events
          </span>
        </div>
        <div className="flex items-center gap-2 text-[11px] text-faint">
          <StatusDot status="offline" />
          Waiting for runtime
        </div>
      </div>

      <section className="overflow-hidden rounded-xl border border-line bg-surface">
        <header className="flex h-10 items-center justify-between border-b border-line px-4">
          <div className="flex items-center gap-2 font-mono text-[11px] text-faint">
            <Terminal className="size-3.5" strokeWidth={1.6} />
            live.log
          </div>
          <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
            Idle
          </span>
        </header>

        <div className="flex min-h-[320px] flex-col items-center justify-center px-6 py-12 text-center">
          <div className="mb-4 flex size-12 items-center justify-center rounded-xl border border-line bg-bg">
            <Terminal className="size-6 text-muted" strokeWidth={1.4} />
          </div>
          <h2 className="text-sm font-medium text-ink">No activity yet</h2>
          <p className="mt-1.5 max-w-sm text-xs leading-5 text-muted">
            Tor bootstrap progress, descriptor updates, service events, and tunnel output
            will appear here.
          </p>
        </div>
      </section>

      <p className="mt-4 text-[11px] leading-5 text-faint">
        Private keys, bridge lines, and tunnel tokens are removed from the event stream.
      </p>
    </div>
  );
}
