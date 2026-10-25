import { ArrowRight, Check, CircleAlert, RefreshCw } from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";
import { useRuntimeStore } from "@/stores/runtime";

export function RuntimeSetup({ onContinue }: { onContinue: () => void }) {
  const status = useRuntimeStore((state) => state.status);
  const busy = useRuntimeStore((state) => state.busy);
  const error = useRuntimeStore((state) => state.error);
  const repair = useRuntimeStore((state) => state.repair);

  return (
    <div className="flex min-h-screen items-center justify-center bg-bg px-6 py-10 text-ink">
      <div className="w-full max-w-xl">
        <div className="flex items-center gap-3">
          <div className="flex size-11 items-center justify-center rounded-xl border border-line bg-surface">
            <OnionMark className="size-6" />
          </div>
          <div>
            <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-faint">
              OnionHoster
            </p>
            <h1 className="mt-0.5 text-lg font-medium tracking-[-0.02em]">
              Runtime setup
            </h1>
          </div>
        </div>

        <p className="mt-5 max-w-lg text-[13px] leading-6 text-muted">
          OnionHoster needs its bundled Tor runtime and cloudflared helper before
          it can publish onion services or tunnels. This setup only appears when
          a component is missing or incomplete.
        </p>

        <div className="mt-6 overflow-hidden rounded-xl border border-line bg-surface">
          <div className="border-b border-line px-4 py-3">
            <h2 className="text-[13px] font-medium">Components</h2>
          </div>
          <div className="divide-y divide-line">
            {(status?.components ?? []).map((component) => (
              <div
                key={component.name}
                className="flex items-center justify-between gap-4 px-4 py-3"
              >
                <div className="flex min-w-0 items-center gap-2.5">
                  {component.installed ? (
                    <Check className="size-4 shrink-0 text-ink" strokeWidth={1.8} />
                  ) : (
                    <CircleAlert
                      className="size-4 shrink-0 text-ink"
                      strokeWidth={1.6}
                    />
                  )}
                  <div className="min-w-0">
                    <p className="text-[12px] font-medium text-ink">
                      {component.name}
                    </p>
                    <p className="mt-0.5 truncate font-mono text-[10px] text-faint">
                      {component.path ?? "Not found"}
                    </p>
                  </div>
                </div>
                <span className="shrink-0 font-mono text-[10px] text-muted">
                  {component.installed
                    ? component.version ?? "installed"
                    : "missing"}
                </span>
              </div>
            ))}
            {!status ? (
              <div className="px-4 py-3 text-[12px] text-muted">
                Checking the bundled runtime…
              </div>
            ) : null}
          </div>
        </div>

        {error ? (
          <p className="mt-4 rounded-lg border border-line bg-surface px-3 py-2.5 text-[12px] leading-5 text-muted">
            {error}
          </p>
        ) : (
          <p className="mt-4 text-[11px] leading-5 text-faint">
            Repair checks the runtime and restores any missing or invalid
            components. If repair cannot recover a component, reinstall
            OnionHoster from the latest package.
          </p>
        )}

        <div className="mt-6 flex flex-wrap items-center justify-between gap-3">
          <Button variant="ghost" size="sm" onClick={onContinue}>
            Continue without Tor
            <ArrowRight className="size-3.5" strokeWidth={1.6} />
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={busy}
            onClick={() => void repair()}
          >
            <RefreshCw
              className={busy ? "size-3.5 animate-spin" : "size-3.5"}
              strokeWidth={1.8}
            />
            {busy ? "Repairing" : "Repair runtime"}
          </Button>
        </div>

        <div className="mt-8 flex gap-2.5 rounded-lg border border-line bg-surface p-3">
          <CircleAlert className="mt-0.5 size-4 shrink-0 text-muted" strokeWidth={1.6} />
          <p className="text-[11px] leading-5 text-muted">
            OnionHoster does not install distro-specific Tor packages. It uses
            the runtime bundled with the app and verifies the release artifacts
            when the app is built.
          </p>
        </div>
      </div>
    </div>
  );
}
