import { Download, Loader2, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { useRuntimeStore } from "@/stores/runtime";
import { useUpdateStore } from "@/stores/updates";

export function UpdateBanner() {
  const available = useUpdateStore((state) => state.available);
  const installing = useUpdateStore((state) => state.installing);
  const progress = useUpdateStore((state) => state.progress);
  const error = useUpdateStore((state) => state.error);
  const install = useUpdateStore((state) => state.install);
  const dismiss = useUpdateStore((state) => state.dismiss);
  const installType = useRuntimeStore(
    (state) => state.status?.installType ?? "tarball",
  );

  if (!available) {
    return null;
  }

  const isAppImage = installType === "appimage";

  function subtitle(): string {
    if (isAppImage) {
      return `You are running ${available!.currentVersion}. Updates are signature-verified before installation.`;
    }
    return `You are running ${available!.currentVersion}. Copy the update command from Settings to install.`;
  }

  return (
    <div className="flex shrink-0 items-start gap-3 border-b border-line bg-surface px-6 py-2.5">
      <Download className="mt-0.5 size-4 shrink-0 text-ink" strokeWidth={1.6} />
      <div className="min-w-0 flex-1">
        <p className="text-[12px] font-medium text-ink">
          OnionHoster {available.version} is available.
        </p>
        <p className="mt-0.5 truncate text-[11px] text-muted">
          {subtitle()}
          {error ? ` ${error}` : ""}
        </p>
      </div>
      {isAppImage ? (
        <Button
          variant="primary"
          size="sm"
          disabled={installing}
          onClick={() => void install()}
        >
          {installing ? (
            <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
          ) : (
            <Download className="size-3.5" strokeWidth={1.8} />
          )}
          {installing
            ? progress === null
              ? "Installing"
              : `${progress}%`
            : "Install update"}
        </Button>
      ) : null}
      <Button
        variant="ghost"
        size="icon"
        onClick={dismiss}
        aria-label="Dismiss update"
        disabled={installing}
      >
        <X className="size-4" strokeWidth={1.6} />
      </Button>
    </div>
  );
}
