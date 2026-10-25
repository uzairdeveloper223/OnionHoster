export type ServiceState =
  | "stopped"
  | "waiting_for_tor"
  | "starting"
  | "running"
  | "error";

export type ProxyScheme = "http" | "https";

export interface StaticSource {
  kind: "static";
  root: string;
  spaFallback: boolean;
  directoryListing: boolean;
}

export interface ProxySource {
  kind: "proxy";
  scheme: ProxyScheme;
  host: string;
  port: number;
  rewriteHost: boolean;
  acceptInvalidCerts: boolean;
}

export interface TcpSource {
  kind: "tcp";
  host: string;
  port: number;
}

export interface DefaultPageSource {
  kind: "default_page";
}

export type ServiceSource =
  | StaticSource
  | ProxySource
  | TcpSource
  | DefaultPageSource;

export interface ServiceOnion {
  virtualPort: number;
  serviceId: string | null;
  pow: boolean;
  maxStreams: number | null;
}

export interface ServiceStats {
  requests: number;
  bytesIn: number;
  bytesOut: number;
  active: number;
  lastRequestAt: number | null;
}

export interface ServiceView {
  id: string;
  name: string;
  source: ServiceSource;
  onion: ServiceOnion;
  autostart: boolean;
  createdAt: number;
  updatedAt: number;
  state: ServiceState;
  onionAddress: string | null;
  localPort: number | null;
  error: string | null;
  stats: ServiceStats;
}

export interface ServiceInput {
  name: string;
  source: ServiceSource;
  virtualPort?: number;
  autostart: boolean;
}

export interface ServiceLog {
  id: string;
  level: string;
  message: string;
  timestampMs: number;
}

export function serviceTypeLabel(source: ServiceSource): string {
  switch (source.kind) {
    case "static":
      return "Static site";
    case "proxy":
      return "Local server";
    case "tcp":
      return "TCP service";
    case "default_page":
      return "OnionHoster page";
  }
}

export function serviceSummary(source: ServiceSource): string {
  switch (source.kind) {
    case "static":
      return source.root;
    case "proxy":
      return `${source.scheme}://${source.host}:${source.port}`;
    case "tcp":
      return `tcp://${source.host}:${source.port}`;
    case "default_page":
      return "Built-in OnionHoster page";
  }
}

export function serviceAddress(service: ServiceView): string | null {
  if (!service.onionAddress) {
    return null;
  }

  if (service.source.kind === "tcp") {
    return `${service.onionAddress}:${service.onion.virtualPort}`;
  }

  return service.onion.virtualPort === 80
    ? `http://${service.onionAddress}`
    : `http://${service.onionAddress}:${service.onion.virtualPort}`;
}

export function serviceStateLabel(service: ServiceView): string {
  switch (service.state) {
    case "running":
      return "Published";
    case "starting":
      return "Starting";
    case "waiting_for_tor":
      return "Waiting for Tor";
    case "error":
      return "Error";
    default:
      return "Stopped";
  }
}

export function serviceTone(state: ServiceState): "offline" | "starting" | "online" | "error" {
  switch (state) {
    case "running":
      return "online";
    case "starting":
    case "waiting_for_tor":
      return "starting";
    case "error":
      return "error";
    default:
      return "offline";
  }
}

export function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
