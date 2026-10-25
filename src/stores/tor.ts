import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

export type TorPhase = "stopped" | "starting" | "bootstrapping" | "ready" | "error";

export interface TorStatus {
  state: TorPhase;
  bootstrap: number;
  summary: string;
  version: string | null;
  error: string | null;
  startedAt: number | null;
}

export interface TorLogEntry {
  id: number;
  level: string;
  message: string;
  timestampMs: number;
}

interface TorStore {
  status: TorStatus;
  logs: TorLogEntry[];
  bridgeLines: string;
  hydrated: boolean;
  busy: boolean;
  hydrate: () => Promise<void>;
  setBridgeLines: (value: string) => void;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  clearLogs: () => void;
}

const defaultStatus: TorStatus = {
  state: "stopped",
  bootstrap: 0,
  summary: "Tor is stopped.",
  version: null,
  error: null,
  startedAt: null,
};

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const BRIDGE_STORAGE_KEY = "onionhoster.tor.bridges";
let listenersAttached = false;
let localLogId = 1;

function readBridgeLines() {
  if (typeof window === "undefined") {
    return "";
  }
  try {
    return window.localStorage.getItem(BRIDGE_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

function storeBridgeLines(value: string) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    if (value) {
      window.localStorage.setItem(BRIDGE_STORAGE_KEY, value);
    } else {
      window.localStorage.removeItem(BRIDGE_STORAGE_KEY);
    }
  } catch {
    // The bridge lines still work for this session if storage is unavailable.
  }
}

function parseBridgeLines(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

function localLog(level: string, message: string): TorLogEntry {
  return {
    id: -localLogId++,
    level,
    message,
    timestampMs: Date.now(),
  };
}

async function attachListeners() {
  if (!isTauri || listenersAttached) {
    return;
  }

  listenersAttached = true;

  await listen<TorStatus>("tor-status", (event) => {
    useTorStore.setState({ status: event.payload });
  });

  await listen<TorLogEntry>("tor-log", (event) => {
    useTorStore.setState((state) => ({
      logs: [...state.logs, event.payload].slice(-500),
    }));
  });
}

export const useTorStore = create<TorStore>((set, get) => ({
  status: defaultStatus,
  logs: [],
  bridgeLines: readBridgeLines(),
  hydrated: false,
  busy: false,

  hydrate: async () => {
    if (get().hydrated) {
      return;
    }

    if (!isTauri) {
      set({ hydrated: true });
      return;
    }

    try {
      await attachListeners();
      const [status, logs] = await Promise.all([
        invoke<TorStatus>("get_tor_status"),
        invoke<TorLogEntry[]>("get_tor_logs"),
      ]);
      set({ status, logs, hydrated: true });
    } catch (error) {
      set({
        hydrated: true,
        logs: [
          localLog("error", `Could not read the Tor runtime state: ${String(error)}`),
        ],
      });
    }
  },

  setBridgeLines: (value) => {
    storeBridgeLines(value);
    set({ bridgeLines: value });
  },

  start: async () => {
    if (get().busy) {
      return;
    }

    if (!isTauri) {
      set((state) => ({
        logs: [
          ...state.logs,
          localLog("warn", "The Tor runtime is only available in the desktop app."),
        ].slice(-500),
      }));
      return;
    }

    set({ busy: true });
    try {
      await invoke("start_tor", {
        config: { bridgeLines: parseBridgeLines(get().bridgeLines) },
      });
    } catch (error) {
      set((state) => ({
        logs: [
          ...state.logs,
          localLog("error", `Could not start Tor: ${String(error)}`),
        ].slice(-500),
      }));
    } finally {
      set({ busy: false });
    }
  },

  stop: async () => {
    if (get().busy) {
      return;
    }

    if (!isTauri) {
      return;
    }

    set({ busy: true });
    try {
      await invoke("stop_tor");
    } catch (error) {
      set((state) => ({
        logs: [
          ...state.logs,
          localLog("error", `Could not stop Tor: ${String(error)}`),
        ].slice(-500),
      }));
    } finally {
      set({ busy: false });
    }
  },

  clearLogs: () => set({ logs: [] }),
}));
