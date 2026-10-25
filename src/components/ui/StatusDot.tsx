import { cn } from "@/lib/utils";

export type Status = "offline" | "starting" | "online" | "error";

const styles: Record<Status, string> = {
  offline: "border border-line-strong bg-transparent",
  starting: "border border-dashed border-ink bg-transparent",
  online: "border border-ink bg-ink",
  error: "border border-ink bg-transparent",
};

export function StatusDot({
  status,
  className,
}: {
  status: Status;
  className?: string;
}) {
  return (
    <span
      className={cn("inline-block size-2 shrink-0 rounded-full", styles[status], className)}
      aria-hidden="true"
    />
  );
}
