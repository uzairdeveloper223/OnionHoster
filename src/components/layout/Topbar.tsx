import { StatusDot } from "@/components/ui/StatusDot";
import { torLabel, torSummary, torTone } from "@/lib/tor";
import { useTorStore } from "@/stores/tor";
import { Button } from "@/components/ui/Button";

export function Topbar({ title, subtitle }: { title: string; subtitle: string }) {
  const status = useTorStore((state) => state.status);
  const busy = useTorStore((state) => state.busy);
  const start = useTorStore((state) => state.start);
  const stop = useTorStore((state) => state.stop);
  const running = ["starting", "bootstrapping", "ready"].includes(status.state);

  return (
    <header className="flex min-h-16 shrink-0 items-center justify-between gap-3 border-b border-line bg-bg px-4 py-3 md:px-6">
      <div className="min-w-0">
        <h1 className="truncate text-[15px] font-medium tracking-[-0.01em] text-ink">
          {title}
        </h1>
        <p className="mt-0.5 hidden truncate text-xs text-muted sm:block">{subtitle}</p>
      </div>

      <div
        className="flex shrink-0 items-center gap-2 rounded-full border border-line bg-surface px-3 py-1.5 text-[11px] text-muted"
        title={torSummary(status)}
      >
        <StatusDot status={torTone(status)} />
        <span>{torLabel(status)}</span>
        <Button variant="ghost" size="sm" className="h-7 px-1.5 text-[11px] md:hidden"
          disabled={busy} onClick={() => void (running ? stop() : start())}>
          {running ? "Stop" : "Start"}
        </Button>
      </div>
    </header>
  );
}
