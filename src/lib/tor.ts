import type { TorStatus } from "@/stores/tor";

export type TorTone = "offline" | "starting" | "online" | "error";

export function torTone(status: TorStatus): TorTone {
  switch (status.state) {
    case "ready":
      return "online";
    case "starting":
    case "bootstrapping":
      return "starting";
    case "error":
      return "error";
    default:
      return "offline";
  }
}

export function torLabel(status: TorStatus): string {
  switch (status.state) {
    case "ready":
      return "Tor ready";
    case "starting":
      return "Starting Tor";
    case "bootstrapping":
      return `Tor ${status.bootstrap}%`;
    case "error":
      return "Tor error";
    default:
      return "Tor stopped";
  }
}

export function torSummary(status: TorStatus): string {
  if (status.state === "error") {
    return status.error ?? "The Tor runtime stopped with an error.";
  }
  if (status.state === "ready") {
    return status.version ? `Tor ${status.version} is ready.` : "Tor is ready.";
  }
  return status.summary;
}

export function formatLogTime(timestampMs: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(timestampMs));
}
