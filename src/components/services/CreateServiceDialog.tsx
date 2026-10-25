import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  ArrowRight,
  Cable,
  Check,
  Circle,
  Copy,
  FileCode2,
  Download,
  FileText,
  FolderOpen,
  FolderUp,
  KeyRound,
  Loader2,
  MessagesSquare,
  Radar,
  Server,
  ShieldCheck,
  X,
} from "lucide-react";
import { OnionMark } from "@/components/brand/OnionMark";
import { Button } from "@/components/ui/Button";
import { formatBytes, serviceAddress, type ServiceSource, type ServiceView } from "@/lib/services";
import { copyText } from "@/lib/clipboard";
import { isTauri } from "@/lib/app";
import { detectLocalPorts, type DetectedPort } from "@/lib/ports";
import { useServiceStore } from "@/stores/services";
import { useTorStore } from "@/stores/tor";
import { cn } from "@/lib/utils";

type Step = "type" | "details" | "publishing";
type ServiceKind = ServiceSource["kind"];

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

function CheckRow({
  checked,
  onChange,
  label,
  description,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description: string;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2.5 rounded-lg border border-line bg-bg px-3 py-2.5">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-0.5 size-3.5 accent-ink"
      />
      <span className="min-w-0">
        <span className="block text-[12px] text-ink">{label}</span>
        <span className="mt-0.5 block text-[11px] leading-4 text-faint">{description}</span>
      </span>
    </label>
  );
}

function StepIcon({ done, active }: { done: boolean; active: boolean }) {
  if (done) {
    return (
      <span className="flex size-5 items-center justify-center rounded-full border border-ink bg-ink text-bg">
        <Check className="size-3" strokeWidth={2} />
      </span>
    );
  }
  if (active) {
    return (
      <span className="flex size-5 items-center justify-center rounded-full border border-ink bg-bg">
        <Loader2 className="size-3 animate-spin" strokeWidth={1.8} />
      </span>
    );
  }
  return (
    <span className="flex size-5 items-center justify-center rounded-full border border-line bg-bg">
      <Circle className="size-2.5 text-faint" strokeWidth={1.6} />
    </span>
  );
}

export function CreateServiceDialog({
  open: isOpen,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [step, setStep] = useState<Step>("type");
  const [kind, setKind] = useState<ServiceKind>("static");
  const [name, setName] = useState("");
  const [root, setRoot] = useState("");
  const [spaFallback, setSpaFallback] = useState(false);
  const [directoryListing, setDirectoryListing] = useState(false);
  const [scheme, setScheme] = useState<"http" | "https">("http");
  const [proxyHost, setProxyHost] = useState("127.0.0.1");
  const [proxyPort, setProxyPort] = useState("3000");
  const [rewriteHost, setRewriteHost] = useState(false);
  const [acceptInvalidCerts, setAcceptInvalidCerts] = useState(false);
  const [tcpHost, setTcpHost] = useState("127.0.0.1");
  const [tcpPort, setTcpPort] = useState("22");
  const [virtualPort, setVirtualPort] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [privateOnion, setPrivateOnion] = useState(false);
  const [pow, setPow] = useState(false);
  const [maxStreams, setMaxStreams] = useState("");
  const [created, setCreated] = useState<ServiceView | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedAuth, setCopiedAuth] = useState(false);
  const [detectedPorts, setDetectedPorts] = useState<DetectedPort[] | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [detectError, setDetectError] = useState<string | null>(null);
  const [dragActive, setDragActive] = useState(false);

  const createService = useServiceStore((state) => state.create);
  const startService = useServiceStore((state) => state.start);
  const live = useServiceStore((state) =>
    created ? state.services.find((service) => service.id === created.id) : undefined,
  );
  const torState = useTorStore((state) => state.status.state);
  const startTor = useTorStore((state) => state.start);

  const service = live ?? created;
  const clientAuth = service?.onion.clientAuth?.[0];
  const clientAuthLine =
    service?.onion.serviceId && clientAuth
      ? `${service.onion.serviceId}:descriptor:x25519:${clientAuth.privateKey}`
      : null;

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    setStep("type");
    setKind("static");
    setName("");
    setRoot("");
    setSpaFallback(false);
    setDirectoryListing(false);
    setScheme("http");
    setProxyHost("127.0.0.1");
    setProxyPort("3000");
    setRewriteHost(false);
    setAcceptInvalidCerts(false);
    setTcpHost("127.0.0.1");
    setTcpPort("22");
    setVirtualPort("");
    setAutostart(false);
    setPrivateOnion(false);
    setPow(false);
    setMaxStreams("");
    setCreated(null);
    setSubmitting(false);
    setError(null);
    setCopied(false);
    setCopiedAuth(false);
    setDetectedPorts(null);
    setDetecting(false);
    setDetectError(null);
    setDragActive(false);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen || !isTauri || kind !== "static" || step !== "details") {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      const stop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDragActive(true);
          return;
        }
        if (event.payload.type === "leave") {
          setDragActive(false);
          return;
        }

        setDragActive(false);
        const path = event.payload.paths[0];
        if (path) {
          setRoot(path);
          setError(null);
        }
      });

      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [isOpen, kind, step]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !submitting) {
        onClose();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, onClose, submitting]);

  if (!isOpen) {
    return null;
  }

  function buildSource(): ServiceSource {
    switch (kind) {
      case "static":
        return {
          kind: "static",
          root,
          spaFallback,
          directoryListing,
        };
      case "proxy":
        return {
          kind: "proxy",
          scheme,
          host: proxyHost,
          port: Number(proxyPort),
          rewriteHost,
          acceptInvalidCerts,
        };
      case "tcp":
        return {
          kind: "tcp",
          host: tcpHost,
          port: Number(tcpPort),
        };
      case "default_page":
        return { kind: "default_page" };
      case "chat":
        return { kind: "chat" };
      case "file_share":
        return { kind: "file_share" };
    }
  }

  function validate(): string | null {
    if (!name.trim()) {
      return "Give the service a name.";
    }
    if (kind === "static" && !root.trim()) {
      return "Choose a folder or HTML file to host.";
    }
    if (kind === "proxy") {
      const port = Number(proxyPort);
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        return "Enter a valid local port between 1 and 65535.";
      }
      if (!proxyHost.trim()) {
        return "Enter the local host.";
      }
    }
    if (kind === "tcp") {
      const port = Number(tcpPort);
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        return "Enter a valid TCP port between 1 and 65535.";
      }
      if (!tcpHost.trim()) {
        return "Enter the local host.";
      }
    }
    if (virtualPort.trim()) {
      const port = Number(virtualPort);
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        return "Enter a valid onion virtual port between 1 and 65535.";
      }
    }
    if (maxStreams.trim()) {
      const streams = Number(maxStreams);
      if (!Number.isInteger(streams) || streams < 1 || streams > 65535) {
        return "Enter a max stream count between 1 and 65535, or leave it empty.";
      }
    }
    return null;
  }

  async function publish() {
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const service = await createService({
        name: name.trim(),
        source: buildSource(),
        virtualPort: virtualPort.trim() ? Number(virtualPort) : undefined,
        autostart,
        pow,
        maxStreams: maxStreams.trim() ? Number(maxStreams) : undefined,
        private: privateOnion,
      });
      setCreated(service);
      setStep("publishing");
      try {
        await startService(service.id);
      } catch (startError) {
        setError(String(startError));
      }
    } catch (createError) {
      setError(String(createError));
    } finally {
      setSubmitting(false);
    }
  }

  async function retry() {
    if (!created) {
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await startService(created.id);
    } catch (startError) {
      setError(String(startError));
    } finally {
      setSubmitting(false);
    }
  }

  async function browseFolder() {
    if (!isTauri) {
      return;
    }
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose a folder to host",
    });
    if (typeof selected === "string") {
      setRoot(selected);
    }
  }

  async function browseFile() {
    if (!isTauri) {
      return;
    }
    const selected = await open({
      multiple: false,
      title: "Choose an HTML file to host",
      filters: [{ name: "HTML", extensions: ["html", "htm"] }],
    });
    if (typeof selected === "string") {
      setRoot(selected);
    }
  }

  async function detectPorts() {
    if (!isTauri || detecting) {
      return;
    }
    setDetecting(true);
    setDetectError(null);
    try {
      setDetectedPorts(await detectLocalPorts());
    } catch (scanError) {
      setDetectError(String(scanError));
    } finally {
      setDetecting(false);
    }
  }

  async function copyAddress() {
    if (!service) {
      return;
    }
    const address = serviceAddress(service);
    if (address && (await copyText(address))) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    }
  }

  async function copyClientAuth() {
    if (!clientAuthLine) {
      return;
    }
    if (await copyText(clientAuthLine)) {
      setCopiedAuth(true);
      window.setTimeout(() => setCopiedAuth(false), 1600);
    }
  }

  function downloadClientAuth() {
    if (!clientAuthLine || !service?.onion.serviceId) {
      return;
    }
    const blob = new Blob([`${clientAuthLine}\n`], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${service.onion.serviceId}.auth_private`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  const state = service?.state;
  const running = state === "running";
  const waiting = state === "waiting_for_tor";
  const processError = state === "error" || Boolean(error);
  const address = service ? serviceAddress(service) : null;
  const localReady = state === "starting" || state === "running";
  const steps = [
    { label: "Configuration saved", done: Boolean(created), active: !created },
    {
      label:
        kind === "tcp"
          ? "TCP target configured"
          : kind === "default_page"
            ? "Built-in page ready"
            : kind === "chat"
              ? "Chat server ready"
              : kind === "file_share"
                ? "File server ready"
                : "Local server ready",
      done: localReady,
      active: Boolean(created) && !localReady && !processError && !waiting,
    },
    {
      label: "Onion service registered",
      done: running,
      active: state === "starting" && !processError,
    },
    {
      label: "Onion address ready",
      done: running && Boolean(address),
      active: running && !address,
    },
  ];

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
        aria-labelledby="create-service-title"
        className="panel flex max-h-[90vh] w-full max-w-xl flex-col overflow-hidden rounded-2xl shadow-2xl"
      >
        <header className="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
          <div>
            <h2 id="create-service-title" className="text-sm font-medium text-ink">
              {step === "publishing" ? "Publishing service" : "New service"}
            </h2>
            <p className="mt-0.5 text-xs text-muted">
              {step === "type"
                ? "Choose what OnionHoster should publish."
                : step === "details"
                  ? kind === "default_page" ||
                    kind === "chat" ||
                    kind === "file_share"
                    ? "Configure the onion port and service options."
                    : "Configure the local source and onion port."
                  : "The service is being prepared and registered with Tor."}
            </p>
          </div>
          <Button
            size="icon"
            variant="ghost"
            disabled={submitting && !created}
            onClick={onClose}
            aria-label="Close"
          >
            <X className="size-4" strokeWidth={1.6} />
          </Button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          {step === "type" ? (
            <div className="grid gap-2">
              {(
                [
                  {
                    kind: "static" as const,
                    icon: FolderOpen,
                    title: "Static site",
                    description: "Serve a folder or a single HTML file from disk.",
                  },
                  {
                    kind: "proxy" as const,
                    icon: Server,
                    title: "Local server",
                    description: "Reverse proxy a running HTTP or HTTPS server.",
                  },
                  {
                    kind: "tcp" as const,
                    icon: Cable,
                    title: "TCP service",
                    description: "Map a raw TCP port, such as SSH, through Tor.",
                  },
                  {
                    kind: "default_page" as const,
                    icon: FileText,
                    title: "OnionHoster default page",
                    description:
                      "Serve a built-in page about the project, with logo, links, and favicon.",
                  },
                  {
                    kind: "chat" as const,
                    icon: MessagesSquare,
                    title: "Public chat",
                    description:
                      "One public room with anonymous names, media previews, and stored history.",
                  },
                  {
                    kind: "file_share" as const,
                    icon: FolderUp,
                    title: "File sharing",
                    description:
                      "A public upload and download space with previews and video streaming.",
                  },
                ] satisfies {
                  kind: ServiceKind;
                  icon: typeof FolderOpen;
                  title: string;
                  description: string;
                }[]
              ).map((option) => {
                const Icon = option.icon;
                const active = kind === option.kind;
                return (
                  <button
                    key={option.kind}
                    type="button"
                    onClick={() => setKind(option.kind)}
                    className={cn(
                      "flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-colors",
                      active
                        ? "border-ink bg-hover"
                        : "border-line bg-bg hover:border-line-strong",
                    )}
                  >
                    <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-surface">
                      <Icon className="size-4 text-muted" strokeWidth={1.6} />
                    </span>
                    <span className="min-w-0">
                      <span className="block text-[13px] font-medium text-ink">
                        {option.title}
                      </span>
                      <span className="mt-0.5 block text-[11px] leading-4 text-muted">
                        {option.description}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          ) : null}

          {step === "details" ? (
            <div className="space-y-3.5">
              <Field label="Service name">
                <input
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder={
                    kind === "static"
                      ? "Portfolio"
                      : kind === "proxy"
                        ? "Development server"
                        : kind === "tcp"
                          ? "SSH"
                          : kind === "chat"
                            ? "Community chat"
                            : kind === "file_share"
                              ? "Shared files"
                              : "OnionHoster page"
                  }
                  maxLength={80}
                  autoFocus
                  className={inputClass}
                />
              </Field>

              {kind === "static" ? (
                <>
                  <Field label="Folder or HTML file">
                    <div className="flex gap-1.5">
                      <input
                        value={root}
                        onChange={(event) => setRoot(event.target.value)}
                        placeholder="/home/you/site"
                        className={cn(inputClass, "font-mono text-[12px]")}
                      />
                      {isTauri ? (
                        <>
                          <Button
                            size="icon"
                            variant="secondary"
                            className="h-9"
                            onClick={() => void browseFolder()}
                            aria-label="Choose folder"
                            title="Choose folder"
                          >
                            <FolderOpen className="size-4" strokeWidth={1.6} />
                          </Button>
                          <Button
                            size="icon"
                            variant="secondary"
                            className="h-9"
                            onClick={() => void browseFile()}
                            aria-label="Choose HTML file"
                            title="Choose HTML file"
                          >
                            <FileCode2 className="size-4" strokeWidth={1.6} />
                          </Button>
                        </>
                      ) : null}
                    </div>
                  </Field>
                  <p
                    className={cn(
                      "text-[11px] leading-4 transition-colors",
                      dragActive ? "text-ink" : "text-faint",
                    )}
                  >
                    {dragActive
                      ? "Drop the folder or HTML file to use it."
                      : "You can also drop a folder or HTML file onto the window."}
                  </p>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <CheckRow
                      checked={spaFallback}
                      onChange={setSpaFallback}
                      label="SPA fallback"
                      description="Serve index.html for unknown paths."
                    />
                    <CheckRow
                      checked={directoryListing}
                      onChange={setDirectoryListing}
                      label="Directory listing"
                      description="Show an index when no index.html exists."
                    />
                  </div>
                </>
              ) : null}

              {kind === "proxy" ? (
                <>
                  <div className="grid grid-cols-[5.5rem_1fr_6rem] gap-2">
                    <Field label="Scheme">
                      <div className="flex h-9 rounded-md border border-line bg-bg p-0.5">
                        {(["http", "https"] as const).map((value) => (
                          <button
                            key={value}
                            type="button"
                            onClick={() => setScheme(value)}
                            className={cn(
                              "flex-1 rounded px-1 font-mono text-[10px] uppercase transition-colors",
                              scheme === value
                                ? "bg-ink text-bg"
                                : "text-faint hover:text-ink",
                            )}
                          >
                            {value}
                          </button>
                        ))}
                      </div>
                    </Field>
                    <Field label="Local host">
                      <input
                        value={proxyHost}
                        onChange={(event) => setProxyHost(event.target.value)}
                        placeholder="127.0.0.1"
                        className={cn(inputClass, "font-mono text-[12px]")}
                      />
                    </Field>
                    <Field label="Port">
                      <input
                        value={proxyPort}
                        onChange={(event) => setProxyPort(event.target.value)}
                        inputMode="numeric"
                        placeholder="3000"
                        className={cn(inputClass, "font-mono text-[12px]")}
                      />
                    </Field>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={detecting || !isTauri}
                      onClick={() => void detectPorts()}
                    >
                      {detecting ? (
                        <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
                      ) : (
                        <Radar className="size-3.5" strokeWidth={1.6} />
                      )}
                      {detecting ? "Scanning" : "Detect local ports"}
                    </Button>
                    {detectedPorts?.map((detected) => (
                      <button
                        key={detected.port}
                        type="button"
                        onClick={() => {
                          setProxyPort(String(detected.port));
                          setScheme(detected.scheme);
                        }}
                        className={cn(
                          "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 font-mono text-[10px] transition-colors",
                          proxyPort === String(detected.port)
                            ? "border-line-strong bg-ink text-bg"
                            : "border-line bg-bg text-muted hover:border-line-strong hover:text-ink",
                        )}
                      >
                        {detected.port}
                        <span className="uppercase opacity-70">{detected.scheme}</span>
                      </button>
                    ))}
                    {detectedPorts?.length === 0 ? (
                      <span className="text-[11px] text-faint">
                        No common local ports are open.
                      </span>
                    ) : null}
                  </div>
                  {detectError ? (
                    <p className="text-[11px] leading-4 text-muted">{detectError}</p>
                  ) : null}
                  <div className="grid gap-2 sm:grid-cols-2">
                    <CheckRow
                      checked={rewriteHost}
                      onChange={setRewriteHost}
                      label="Rewrite Host"
                      description="Send the upstream host instead of the onion host."
                    />
                    <CheckRow
                      checked={acceptInvalidCerts}
                      onChange={setAcceptInvalidCerts}
                      label="Accept invalid certificates"
                      description="Allow self-signed HTTPS upstreams."
                    />
                  </div>
                </>
              ) : null}

              {kind === "tcp" ? (
                <div className="grid grid-cols-[1fr_7rem] gap-2">
                  <Field label="Local host">
                    <input
                      value={tcpHost}
                      onChange={(event) => setTcpHost(event.target.value)}
                      placeholder="127.0.0.1"
                      className={cn(inputClass, "font-mono text-[12px]")}
                    />
                  </Field>
                  <Field label="Port">
                    <input
                      value={tcpPort}
                      onChange={(event) => setTcpPort(event.target.value)}
                      inputMode="numeric"
                      placeholder="22"
                      className={cn(inputClass, "font-mono text-[12px]")}
                    />
                  </Field>
                </div>
              ) : null}

              {kind === "default_page" ? (
                <div className="rounded-xl border border-line bg-bg p-3.5">
                  <div className="flex items-start gap-3">
                    <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-line bg-bg">
                      <OnionMark className="size-5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-[12px] font-medium text-ink">
                        Built-in OnionHoster page
                      </div>
                      <p className="mt-1 text-[11px] leading-4 text-muted">
                        Serves a monochrome page with the OnionHoster logo,
                        favicon, project details, author links, and repository
                        link. No local files or server are needed.
                      </p>
                    </div>
                  </div>
                  <ul className="mt-3 grid gap-1.5 border-t border-line pt-3 font-mono text-[10px] text-faint sm:grid-cols-2">
                    <li>About the app</li>
                    <li>About the author</li>
                    <li>Repository and license</li>
                    <li>Privacy and Tor details</li>
                  </ul>
                </div>
              ) : null}

              {kind === "chat" ? (
                <div className="rounded-xl border border-line bg-bg p-3.5">
                  <div className="flex items-start gap-3">
                    <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-line bg-bg">
                      <MessagesSquare className="size-4 text-muted" strokeWidth={1.6} />
                    </div>
                    <div className="min-w-0">
                      <div className="text-[12px] font-medium text-ink">
                        Public chat room
                      </div>
                      <p className="mt-1 text-[11px] leading-4 text-muted">
                        One room. Visitors get an anonymous#1234 identity and can
                        set a display name. Messages and media stay on this
                        machine across service and app restarts.
                      </p>
                    </div>
                  </div>
                  <ul className="mt-3 grid gap-1.5 border-t border-line pt-3 font-mono text-[10px] text-faint sm:grid-cols-2">
                    <li>25 MB per file</li>
                    <li>1 GB storage quota</li>
                    <li>Compact image and video previews</li>
                    <li>Not included in address backups</li>
                  </ul>
                </div>
              ) : null}

              {kind === "file_share" ? (
                <div className="rounded-xl border border-line bg-bg p-3.5">
                  <div className="flex items-start gap-3">
                    <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-line bg-bg">
                      <FolderUp className="size-4 text-muted" strokeWidth={1.6} />
                    </div>
                    <div className="min-w-0">
                      <div className="text-[12px] font-medium text-ink">
                        Public file sharing
                      </div>
                      <p className="mt-1 text-[11px] leading-4 text-muted">
                        A public upload and download space with image previews
                        and browser video playback. Files stay on this machine
                        across service and app restarts.
                      </p>
                    </div>
                  </div>
                  <ul className="mt-3 grid gap-1.5 border-t border-line pt-3 font-mono text-[10px] text-faint sm:grid-cols-2">
                    <li>50 MB per file</li>
                    <li>2 GB storage quota</li>
                    <li>Video watch and download</li>
                    <li>Not included in address backups</li>
                  </ul>
                </div>
              ) : null}

              <div className="grid gap-2 sm:grid-cols-[1fr_1fr]">
                <Field
                  label="Onion virtual port"
                  hint={virtualPort.trim() ? undefined : "auto"}
                >
                  <input
                    value={virtualPort}
                    onChange={(event) => setVirtualPort(event.target.value)}
                    inputMode="numeric"
                    placeholder={
                      kind === "tcp" ? tcpPort || "22" : "80"
                    }
                    className={cn(inputClass, "font-mono text-[12px]")}
                  />
                </Field>
                <div className="flex items-end">
                  <CheckRow
                    checked={autostart}
                    onChange={setAutostart}
                    label="Autostart"
                    description="Publish with Tor after restart."
                  />
                </div>
              </div>

              <div className="grid gap-2 sm:grid-cols-2">
                <CheckRow
                  checked={privateOnion}
                  onChange={setPrivateOnion}
                  label="Private onion"
                  description="Require a client authorization key."
                />
                <CheckRow
                  checked={pow}
                  onChange={setPow}
                  label="Proof-of-Work defenses"
                  description="Ask browsers to solve a small challenge under load."
                />
              </div>

              <Field
                label="Max streams per circuit"
                hint={maxStreams.trim() ? undefined : "unlimited"}
              >
                <input
                  value={maxStreams}
                  onChange={(event) => setMaxStreams(event.target.value)}
                  inputMode="numeric"
                  placeholder="0 or empty for unlimited"
                  className={cn(inputClass, "font-mono text-[12px]")}
                />
              </Field>

              {privateOnion ? (
                <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                  A client key will be generated when the service is created. Add
                  it to Tor Browser before opening the onion address.
                </p>
              ) : null}

              {error ? (
                <p className="rounded-lg border border-line bg-bg px-3 py-2 text-[11px] leading-4 text-muted">
                  {error}
                </p>
              ) : null}
            </div>
          ) : null}

          {step === "publishing" ? (
            <div>
              <div className="rounded-xl border border-line bg-bg p-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="truncate text-[13px] font-medium text-ink">
                      {service?.name ?? name}
                    </h3>
                    <p className="mt-0.5 font-mono text-[10px] text-faint">
                      {kind === "static"
                        ? "Static site"
                        : kind === "proxy"
                          ? "Local server"
                          : kind === "tcp"
                            ? "TCP service"
                            : kind === "chat"
                              ? "Public chat"
                              : kind === "file_share"
                                ? "File sharing"
                                : "OnionHoster page"}
                    </p>
                  </div>
                  <span className="rounded-full border border-line px-2 py-1 font-mono text-[10px] text-faint">
                    {running
                      ? "Published"
                      : waiting
                        ? "Waiting for Tor"
                        : processError
                          ? "Error"
                          : "Publishing"}
                  </span>
                </div>

                <ol className="mt-4 space-y-2.5">
                  {steps.map((item) => (
                    <li key={item.label} className="flex items-center gap-2.5">
                      <StepIcon done={item.done} active={item.active} />
                      <span
                        className={cn(
                          "text-[12px]",
                          item.done ? "text-ink" : "text-faint",
                        )}
                      >
                        {item.label}
                      </span>
                    </li>
                  ))}
                </ol>
              </div>

              {address ? (
                <div className="mt-3 flex items-center gap-1.5 rounded-lg border border-line bg-bg px-2.5 py-2">
                  <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-ink">
                    {address}
                  </code>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-6"
                    onClick={() => void copyAddress()}
                    aria-label="Copy onion address"
                  >
                    {copied ? (
                      <Check className="size-3.5" strokeWidth={1.8} />
                    ) : (
                      <Copy className="size-3.5" strokeWidth={1.6} />
                    )}
                  </Button>
                </div>
              ) : null}

              {service?.onion.private && clientAuth ? (
                <div className="mt-3 rounded-lg border border-line bg-bg p-3">
                  <div className="flex items-start gap-2.5">
                    <ShieldCheck className="mt-0.5 size-4 shrink-0 text-ink" strokeWidth={1.6} />
                    <div className="min-w-0">
                      <div className="text-[12px] font-medium text-ink">
                        Client authorization
                      </div>
                      <p className="mt-1 text-[11px] leading-4 text-muted">
                        Save this key and add it to Tor Browser before opening
                        the onion address.
                      </p>
                    </div>
                  </div>
                  {clientAuthLine ? (
                    <code className="mt-2 block break-all rounded-md border border-line bg-surface px-2.5 py-2 font-mono text-[10px] leading-4 text-ink">
                      {clientAuthLine}
                    </code>
                  ) : (
                    <p className="mt-2 text-[11px] leading-4 text-faint">
                      The key file will be ready once the onion address is published.
                    </p>
                  )}
                  <div className="mt-2 flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={!clientAuthLine}
                      onClick={() => void copyClientAuth()}
                    >
                      {copiedAuth ? (
                        <Check className="size-3.5" strokeWidth={1.8} />
                      ) : (
                        <KeyRound className="size-3.5" strokeWidth={1.6} />
                      )}
                      {copiedAuth ? "Copied" : "Copy key"}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={!clientAuthLine}
                      onClick={downloadClientAuth}
                    >
                      <Download className="size-3.5" strokeWidth={1.6} />
                      .auth_private
                    </Button>
                  </div>
                </div>
              ) : null}

              {waiting ? (
                <div className="mt-3 rounded-lg border border-line bg-bg px-3 py-2.5">
                  <p className="text-[11px] leading-4 text-muted">
                    The service is queued. Start Tor to publish it automatically.
                  </p>
                  <Button
                    size="sm"
                    className="mt-2"
                    disabled={torState !== "stopped" && torState !== "error"}
                    onClick={() => {
                      void startTor();
                    }}
                  >
                    Start Tor
                  </Button>
                </div>
              ) : null}

              {processError ? (
                <div className="mt-3 rounded-lg border border-line bg-bg px-3 py-2.5">
                  <p className="break-words text-[11px] leading-4 text-muted">
                    {error ?? service?.error ?? "The service could not be published."}
                  </p>
                  <Button
                    size="sm"
                    className="mt-2"
                    disabled={submitting}
                    onClick={() => void retry()}
                  >
                    {submitting ? (
                      <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
                    ) : null}
                    Retry
                  </Button>
                </div>
              ) : null}

              {running && service ? (
                <div className="mt-3 flex items-center justify-between gap-3 rounded-lg border border-line bg-bg px-3 py-2.5">
                  <p className="text-[11px] text-muted">
                    Requests: {service.stats.requests.toLocaleString()} · Out:{" "}
                    {formatBytes(service.stats.bytesOut)}
                  </p>
                  <span className="font-mono text-[10px] text-faint">
                    local :{service.localPort ?? "tcp"}
                  </span>
                </div>
              ) : null}
            </div>
          ) : null}
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-line px-5 py-3.5">
          {step === "type" ? (
            <>
              <span className="font-mono text-[10px] text-faint">Step 1 of 2</span>
              <Button onClick={() => setStep("details")}>
                Continue
                <ArrowRight className="size-3.5" strokeWidth={1.8} />
              </Button>
            </>
          ) : step === "details" ? (
            <>
              <Button variant="ghost" onClick={() => setStep("type")}>
                <ArrowLeft className="size-3.5" strokeWidth={1.8} />
                Back
              </Button>
              <Button disabled={submitting} onClick={() => void publish()}>
                {submitting ? (
                  <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
                ) : null}
                Create and publish
              </Button>
            </>
          ) : (
            <>
              <Button variant="ghost" onClick={onClose}>
                Close
              </Button>
              {running ? (
                <Button onClick={onClose}>
                  <Check className="size-3.5" strokeWidth={1.8} />
                  Done
                </Button>
              ) : null}
            </>
          )}
        </footer>
      </div>
    </div>
  );
}
