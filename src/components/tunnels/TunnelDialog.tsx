import { useEffect, useState } from "react";
import {
  Cloud,
  Loader2,
  ShieldAlert,
  Terminal,
  Waypoints,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { serviceStateLabel, serviceTypeLabel } from "@/lib/services";
import {
  tunnelProviderLabel,
  type TunnelProvider,
  type TunnelView,
} from "@/lib/tunnels";
import { useServiceStore } from "@/stores/services";
import { useTunnelStore } from "@/stores/tunnels";
import { cn } from "@/lib/utils";

const inputClass =
  "h-9 w-full rounded-md border border-line bg-bg px-2.5 text-[13px] text-ink outline-none transition-colors placeholder:text-faint focus:border-line-strong";

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 flex items-baseline justify-between gap-3">
        <span className="text-[11px] font-medium text-muted">{label}</span>
        {hint ? <span className="font-mono text-[10px] text-faint">{hint}</span> : null}
      </span>
      {children}
    </label>
  );
}

const providers: {
  id: TunnelProvider;
  icon: typeof Cloud;
  title: string;
  description: string;
  meta: string;
}[] = [
  {
    id: "cloudflare_quick",
    icon: Cloud,
    title: "Cloudflare Quick",
    description: "No account. A temporary public URL for testing and demos.",
    meta: "TEMP",
  },
  {
    id: "cloudflare_named",
    icon: Waypoints,
    title: "Cloudflare Named",
    description: "Use a token and a hostname on a domain you control.",
    meta: "TOKEN",
  },
  {
    id: "custom",
    icon: Terminal,
    title: "Custom command",
    description: "Run another tunnel client with a target template.",
    meta: "ADVANCED",
  },
];

export function TunnelDialog({
  open,
  editing,
  onClose,
}: {
  open: boolean;
  editing: TunnelView | null;
  onClose: () => void;
}) {
  const services = useServiceStore((state) => state.services);
  const createTunnel = useTunnelStore((state) => state.create);
  const updateTunnel = useTunnelStore((state) => state.update);
  const startTunnel = useTunnelStore((state) => state.start);

  const [name, setName] = useState("");
  const [serviceId, setServiceId] = useState("");
  const [provider, setProvider] =
    useState<TunnelProvider>("cloudflare_quick");
  const [token, setToken] = useState("");
  const [publicHostname, setPublicHostname] = useState("");
  const [customCommand, setCustomCommand] = useState("");
  const [expectedUrl, setExpectedUrl] = useState("");
  const [startAfter, setStartAfter] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }

    setName(editing?.name ?? "");
    setServiceId(editing?.serviceId ?? services[0]?.id ?? "");
    setProvider(editing?.provider ?? "cloudflare_quick");
    setToken("");
    setPublicHostname("");
    setCustomCommand("");
    setExpectedUrl("");
    setStartAfter(!editing);
    setSubmitting(false);
    setError(null);
    // Reset only when the dialog opens or the edited tunnel changes. Service
    // polling must not overwrite a form the user is filling in.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing, open]);

  useEffect(() => {
    if (open && !serviceId && services.length > 0) {
      setServiceId(services[0].id);
    }
  }, [open, serviceId, services]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !submitting) {
        onClose();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose, open, submitting]);

  if (!open) {
    return null;
  }

  const selectedService = services.find((service) => service.id === serviceId);
  const selectedIsTcp = selectedService?.source.kind === "tcp";
  const cloudflareWithTcp =
    (provider === "cloudflare_quick" || provider === "cloudflare_named") &&
    selectedIsTcp;

  function validate(): string | null {
    if (!name.trim()) {
      return "Give the tunnel a name.";
    }
    if (!serviceId) {
      return "Choose a service to expose.";
    }
    if (cloudflareWithTcp) {
      return "Cloudflare tunnels support HTTP and HTTPS services only. Use a custom command for TCP.";
    }
    if (provider === "cloudflare_named") {
      if (!editing && !token.trim()) {
        return "Enter the Cloudflare tunnel token.";
      }
      if (!publicHostname.trim()) {
        return "Enter the public hostname configured in Cloudflare.";
      }
    }
    if (provider === "custom") {
      if (!customCommand.trim()) {
        return "Enter the custom tunnel command.";
      }
      if (
        !["{port}", "{target}", "{url}", "{host}"].some((placeholder) =>
          customCommand.includes(placeholder),
        )
      ) {
        return "The custom command must include {port}, {target}, {url}, or {host}.";
      }
    }
    if (expectedUrl.trim() && !/^https?:\/\//i.test(expectedUrl.trim())) {
      return "The public URL must start with http:// or https://.";
    }
    return null;
  }

  async function submit() {
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const input = {
        name: name.trim(),
        serviceId,
        provider,
        token: token.trim() || undefined,
        publicHostname: publicHostname.trim() || undefined,
        customCommand: customCommand.trim() || undefined,
        expectedUrl: expectedUrl.trim() || undefined,
        autostart: false,
      };
      const tunnel = editing
        ? await updateTunnel(editing.id, input)
        : await createTunnel(input);

      if (startAfter) {
        try {
          await startTunnel(tunnel.id);
        } catch {
          // The store keeps the start error for the tunnel banner.
        }
      }
      onClose();
    } catch (submitError) {
      setError(String(submitError));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-[2px]"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !submitting) {
          onClose();
        }
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="tunnel-dialog-title"
        className="panel flex max-h-[90vh] w-full max-w-xl flex-col overflow-hidden rounded-2xl shadow-2xl"
      >
        <header className="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
          <div>
            <h2 id="tunnel-dialog-title" className="text-sm font-medium text-ink">
              {editing ? "Edit tunnel" : "New tunnel"}
            </h2>
            <p className="mt-0.5 text-xs text-muted">
              Expose a running service at a public clearnet URL.
            </p>
          </div>
          <Button
            size="icon"
            variant="ghost"
            disabled={submitting}
            onClick={onClose}
            aria-label="Close"
          >
            <X className="size-4" strokeWidth={1.6} />
          </Button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          <div className="space-y-3.5">
            <Field label="Tunnel name">
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="Public preview"
                maxLength={80}
                autoFocus
                className={inputClass}
              />
            </Field>

            <Field label="Service" hint={selectedService ? serviceTypeLabel(selectedService.source) : undefined}>
              <select
                value={serviceId}
                onChange={(event) => setServiceId(event.target.value)}
                className={cn(inputClass, "appearance-none")}
              >
                {services.length === 0 ? (
                  <option value="">No services yet</option>
                ) : null}
                {services.map((service) => (
                  <option key={service.id} value={service.id}>
                    {service.name} — {serviceStateLabel(service)}
                  </option>
                ))}
              </select>
            </Field>

            {selectedService && selectedService.state !== "running" ? (
              <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                This service is not running. The tunnel can be saved now, but it
                must be started after the service is published.
              </p>
            ) : null}

            <div>
              <div className="mb-1.5 text-[11px] font-medium text-muted">Provider</div>
              <div className="grid gap-2">
                {providers.map((option) => {
                  const Icon = option.icon;
                  const active = provider === option.id;
                  const disabled =
                    selectedIsTcp &&
                    (option.id === "cloudflare_quick" ||
                      option.id === "cloudflare_named");

                  return (
                    <button
                      key={option.id}
                      type="button"
                      disabled={disabled}
                      onClick={() => setProvider(option.id)}
                      className={cn(
                        "flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-40",
                        active
                          ? "border-ink bg-hover"
                          : "border-line bg-bg hover:border-line-strong",
                      )}
                    >
                      <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-surface">
                        <Icon className="size-4 text-muted" strokeWidth={1.6} />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center justify-between gap-2">
                          <span className="text-[13px] font-medium text-ink">
                            {option.title}
                          </span>
                          <span className="font-mono text-[9px] tracking-[0.14em] text-faint">
                            {option.meta}
                          </span>
                        </span>
                        <span className="mt-0.5 block text-[11px] leading-4 text-muted">
                          {option.description}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>

            {provider === "cloudflare_named" ? (
              <>
                <Field
                  label="Tunnel token"
                  hint={editing ? "leave blank to keep saved token" : "required"}
                >
                  <input
                    value={token}
                    onChange={(event) => setToken(event.target.value)}
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    placeholder={editing ? "Saved token" : "eyJhIjoi..."}
                    className={cn(inputClass, "font-mono text-[12px]")}
                  />
                </Field>
                <Field label="Public hostname" hint="example.com">
                  <input
                    value={publicHostname}
                    onChange={(event) => setPublicHostname(event.target.value)}
                    placeholder="app.example.com"
                    spellCheck={false}
                    className={cn(inputClass, "font-mono text-[12px]")}
                  />
                </Field>
                <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                  In Cloudflare, route this hostname to the local target shown on
                  the tunnel card. Named tunnels do not work until that ingress is
                  configured.
                </p>
              </>
            ) : null}

            {provider === "cloudflare_quick" ? (
              <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                Quick tunnels get a random trycloudflare.com address. The address
                changes every time the tunnel starts and Cloudflare provides no
                uptime guarantee.
              </p>
            ) : null}

            {provider === "custom" ? (
              <>
                <Field
                  label="Command template"
                  hint="{host} {port} {target} {url}"
                >
                  <textarea
                    value={customCommand}
                    onChange={(event) => setCustomCommand(event.target.value)}
                    rows={3}
                    spellCheck={false}
                    placeholder="my-tunnel --local {target} --public my.example.com"
                    className="w-full resize-y rounded-lg border border-line bg-bg px-3 py-2.5 font-mono text-[11px] leading-5 text-ink outline-none transition-colors placeholder:text-faint focus:border-line-strong"
                  />
                </Field>
                <Field label="Public URL" hint="optional">
                  <input
                    value={expectedUrl}
                    onChange={(event) => setExpectedUrl(event.target.value)}
                    placeholder="https://my.example.com"
                    spellCheck={false}
                    className={cn(inputClass, "font-mono text-[12px]")}
                  />
                </Field>
                <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                  OnionHoster runs this command with your user account. Only use
                  commands you understand.
                </p>
              </>
            ) : null}

            <label className="flex cursor-pointer items-start gap-2.5 rounded-lg border border-line bg-bg px-3 py-2.5">
              <input
                type="checkbox"
                checked={startAfter}
                onChange={(event) => setStartAfter(event.target.checked)}
                className="mt-0.5 size-3.5 accent-ink"
              />
              <span className="min-w-0">
                <span className="block text-[12px] text-ink">
                  {editing ? "Restart after saving" : "Start after creating"}
                </span>
                <span className="mt-0.5 block text-[11px] leading-4 text-faint">
                  The selected service must already be running.
                </span>
              </span>
            </label>

            <div className="flex gap-2.5 rounded-lg border border-line bg-bg px-3 py-2.5">
              <ShieldAlert
                className="mt-0.5 size-4 shrink-0 text-ink"
                strokeWidth={1.6}
              />
              <p className="text-[11px] leading-4 text-muted">
                Tunnels are public clearnet access and are not anonymous. Do not
                expose private admin panels, local files, or services you do not
                want indexed or attacked.
              </p>
            </div>

            {cloudflareWithTcp ? (
              <p className="text-[11px] leading-4 text-muted">
                Cloudflare cannot publish raw TCP. Choose a custom command or a
                different service.
              </p>
            ) : null}

            {error ? (
              <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                {error}
              </p>
            ) : null}
          </div>
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-line px-5 py-3.5">
          <span className="font-mono text-[10px] text-faint">
            {provider === "cloudflare_named"
              ? "Cloudflare Named"
              : provider === "cloudflare_quick"
                ? "Cloudflare Quick"
                : tunnelProviderLabel(provider)}
          </span>
          <div className="flex gap-2">
            <Button variant="ghost" disabled={submitting} onClick={onClose}>
              Cancel
            </Button>
            <Button disabled={submitting || services.length === 0} onClick={() => void submit()}>
              {submitting ? (
                <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
              ) : null}
              {editing ? "Save tunnel" : "Create tunnel"}
            </Button>
          </div>
        </footer>
      </div>
    </div>
  );
}
