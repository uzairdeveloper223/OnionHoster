import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { isTauri } from "@/lib/app";
import type {
  TunnelDeleted,
  TunnelInput,
  TunnelLog,
  TunnelView,
} from "@/lib/tunnels";

const MAX_LOGS = 500;

interface TunnelStore {
  tunnels: TunnelView[];
  logs: Record<string, TunnelLog[]>;
  hydrated: boolean;
  busy: Record<string, boolean>;
  lastError: string | null;
  hydrate: () => Promise<void>;
  refresh: () => Promise<void>;
  create: (input: TunnelInput) => Promise<TunnelView>;
  update: (id: string, input: TunnelInput) => Promise<TunnelView>;
  start: (id: string) => Promise<TunnelView>;
  stop: (id: string) => Promise<TunnelView>;
  restart: (id: string) => Promise<TunnelView>;
  remove: (id: string) => Promise<void>;
  loadLogs: (id: string) => Promise<void>;
  clearLogs: (id: string) => void;
  clearAllLogs: () => void;
  clearError: () => void;
}

let listenersAttached = false;

function upsert(tunnels: TunnelView[], tunnel: TunnelView): TunnelView[] {
  const index = tunnels.findIndex((item) => item.id === tunnel.id);
  if (index === -1) {
    return [...tunnels, tunnel];
  }

  const next = tunnels.slice();
  next[index] = tunnel;
  return next;
}

async function setBusy(id: string, value: boolean) {
  useTunnelStore.setState((state) => {
    const busy = { ...state.busy };
    if (value) {
      busy[id] = true;
    } else {
      delete busy[id];
    }
    return { busy };
  });
}

async function attachListeners() {
  if (!isTauri || listenersAttached) {
    return;
  }

  listenersAttached = true;

  await listen<TunnelView>("tunnel-status", (event) => {
    useTunnelStore.setState((state) => ({
      tunnels: upsert(state.tunnels, event.payload),
    }));
  });

  await listen<TunnelDeleted>("tunnel-deleted", (event) => {
    useTunnelStore.setState((state) => {
      const logs = { ...state.logs };
      delete logs[event.payload.id];
      return {
        tunnels: state.tunnels.filter((tunnel) => tunnel.id !== event.payload.id),
        logs,
      };
    });
  });

  await listen<TunnelLog>("tunnel-log", (event) => {
    useTunnelStore.setState((state) => {
      const current = state.logs[event.payload.tunnelId] ?? [];
      return {
        logs: {
          ...state.logs,
          [event.payload.tunnelId]: [...current, event.payload].slice(-MAX_LOGS),
        },
      };
    });
  });
}

export const useTunnelStore = create<TunnelStore>((set, get) => ({
  tunnels: [],
  logs: {},
  hydrated: false,
  busy: {},
  lastError: null,

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
      const tunnels = await invoke<TunnelView[]>("get_tunnels");
      set({ tunnels, hydrated: true });
    } catch (error) {
      set({
        hydrated: true,
        lastError: `Could not read the tunnel list: ${String(error)}`,
      });
    }
  },

  refresh: async () => {
    if (!isTauri) {
      return;
    }

    try {
      const tunnels = await invoke<TunnelView[]>("get_tunnels");
      set({ tunnels });
    } catch (error) {
      set({ lastError: `Could not refresh tunnels: ${String(error)}` });
    }
  },

  create: async (input) => {
    set({ lastError: null });
    const tunnel = await invoke<TunnelView>("create_tunnel", { input });
    set((state) => ({ tunnels: upsert(state.tunnels, tunnel) }));
    return tunnel;
  },

  update: async (id, input) => {
    set({ lastError: null });
    const tunnel = await invoke<TunnelView>("update_tunnel", { id, input });
    set((state) => ({ tunnels: upsert(state.tunnels, tunnel) }));
    return tunnel;
  },

  start: async (id) => {
    set({ lastError: null });
    await setBusy(id, true);
    try {
      const tunnel = await invoke<TunnelView>("start_tunnel", { id });
      set((state) => ({ tunnels: upsert(state.tunnels, tunnel) }));
      return tunnel;
    } catch (error) {
      set({ lastError: String(error) });
      throw error;
    } finally {
      await setBusy(id, false);
    }
  },

  stop: async (id) => {
    set({ lastError: null });
    await setBusy(id, true);
    try {
      const tunnel = await invoke<TunnelView>("stop_tunnel", { id });
      set((state) => ({ tunnels: upsert(state.tunnels, tunnel) }));
      return tunnel;
    } catch (error) {
      set({ lastError: String(error) });
      throw error;
    } finally {
      await setBusy(id, false);
    }
  },

  restart: async (id) => {
    set({ lastError: null });
    await setBusy(id, true);
    try {
      await invoke<TunnelView>("stop_tunnel", { id });
      const tunnel = await invoke<TunnelView>("start_tunnel", { id });
      set((state) => ({ tunnels: upsert(state.tunnels, tunnel) }));
      return tunnel;
    } catch (error) {
      set({ lastError: String(error) });
      throw error;
    } finally {
      await setBusy(id, false);
    }
  },

  remove: async (id) => {
    set({ lastError: null });
    await setBusy(id, true);
    try {
      await invoke("delete_tunnel", { id });
      set((state) => {
        const logs = { ...state.logs };
        delete logs[id];
        return {
          tunnels: state.tunnels.filter((tunnel) => tunnel.id !== id),
          logs,
        };
      });
    } catch (error) {
      set({ lastError: String(error) });
      throw error;
    } finally {
      await setBusy(id, false);
    }
  },

  loadLogs: async (id) => {
    if (!isTauri) {
      return;
    }

    try {
      const logs = await invoke<TunnelLog[]>("get_tunnel_logs", { id });
      set((state) => ({ logs: { ...state.logs, [id]: logs.slice(-MAX_LOGS) } }));
    } catch (error) {
      set({ lastError: `Could not read tunnel logs: ${String(error)}` });
    }
  },

  clearLogs: (id) =>
    set((state) => ({
      logs: { ...state.logs, [id]: [] },
    })),

  clearAllLogs: () => set({ logs: {} }),

  clearError: () => set({ lastError: null }),
}));
