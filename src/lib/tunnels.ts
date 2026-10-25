export type TunnelProvider =
  | "cloudflare_quick"
  | "cloudflare_named"
  | "custom";

export type TunnelState = "stopped" | "starting" | "running" | "error";

export interface TunnelView {
  id: string;
  name: string;
  serviceId: string;
  provider: TunnelProvider;
  state: TunnelState;
  publicUrl: string | null;
  localTarget: string | null;
  error: string | null;
  autostart: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface TunnelInput {
  name: string;
  serviceId: string;
  provider: TunnelProvider;
  token?: string;
  publicHostname?: string;
  customCommand?: string;
  expectedUrl?: string;
  autostart: boolean;
}

export interface TunnelLog {
  id: number;
  tunnelId: string;
  level: string;
  message: string;
  timestampMs: number;
}

export interface TunnelDeleted {
  id: string;
}

export function tunnelProviderLabel(provider: TunnelProvider): string {
  switch (provider) {
    case "cloudflare_quick":
      return "Cloudflare Quick";
    case "cloudflare_named":
      return "Cloudflare Named";
    case "custom":
      return "Custom command";
  }
}

export function tunnelProviderDescription(provider: TunnelProvider): string {
  switch (provider) {
    case "cloudflare_quick":
      return "No account. A temporary trycloudflare.com URL.";
    case "cloudflare_named":
      return "Your Cloudflare token and a hostname on your domain.";
    case "custom":
      return "Run your own tunnel client with a target template.";
  }
}

export function tunnelStateLabel(state: TunnelState): string {
  switch (state) {
    case "running":
      return "Running";
    case "starting":
      return "Starting";
    case "error":
      return "Error";
    default:
      return "Stopped";
  }
}

export function tunnelTone(
  state: TunnelState,
): "offline" | "starting" | "online" | "error" {
  switch (state) {
    case "running":
      return "online";
    case "starting":
      return "starting";
    case "error":
      return "error";
    default:
      return "offline";
  }
}

export function tunnelServiceLabel(
  serviceId: string,
  services: { id: string; name: string }[],
): string {
  return services.find((service) => service.id === serviceId)?.name ?? serviceId;
}
