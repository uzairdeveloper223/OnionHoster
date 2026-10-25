import { cn } from "@/lib/utils";

export function ProgressBar({
  value,
  className,
}: {
  value: number;
  className?: string;
}) {
  const percent = Math.max(0, Math.min(100, Math.round(value)));

  return (
    <div
      className={cn("h-1 overflow-hidden rounded-full bg-line", className)}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
    >
      <div
        className="h-full rounded-full bg-ink transition-[width] duration-500 ease-out"
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}
