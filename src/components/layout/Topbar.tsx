import { StatusDot } from "@/components/ui/StatusDot";
import { torLabel, torSummary, torTone } from "@/lib/tor";
import { useTorStore } from "@/stores/tor";

export function Topbar({ title, subtitle }: { title: string; subtitle: string }) {
  const status = useTorStore((state) => state.status);

  return (
    <header className="flex h-16 shrink-0 items-center justify-between gap-6 border-b border-line bg-bg px-6">
      <div className="min-w-0">
        <h1 className="truncate text-[15px] font-medium tracking-[-0.01em] text-ink">
          {title}
        </h1>
        <p className="mt-0.5 truncate text-xs text-muted">{subtitle}</p>
      </div>

      <div
        className="flex shrink-0 items-center gap-2 rounded-full border border-line bg-surface px-3 py-1.5 text-[11px] text-muted"
        title={torSummary(status)}
      >
        <StatusDot status={torTone(status)} />
        <span>{torLabel(status)}</span>
      </div>
    </header>
  );
}
