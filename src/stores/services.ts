import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { isTauri } from "@/lib/app";
import type {
  ServiceInput,
  ServiceLog,
  ServiceView,
} from "@/lib/services";

const MAX_LOGS = 500;

interface ServiceStore {
  services: ServiceView[];
  logs: ServiceLog[];
  hydrated: boolean;
  busy: Record<string, boolean>;
  lastError: string | null;
  hydrate: () => Promise<void>;
  refresh: () => Promise<void>;
  create: (input: ServiceInput) => Promise<ServiceView>;
  update: (id: string, input: ServiceInput) => Promise<ServiceView>;
  start: (id: string) => Promise<ServiceView>;
  stop: (id: string) => Promise<ServiceView>;
  restart: (id: string) => Promise<ServiceView>;
  remove: (id: string) => Promise<void>;
  clearError: () => void;
  clearLogs: () => void;
}

let listenersAttached = false;

function upsert(services: ServiceView[], service: ServiceView): ServiceView[] {
  const index = services.findIndex((item) => item.id === service.id);
  if (index === -1) {
    return [...services, service];
  }

  const next = services.slice();
  next[index] = service;
  return next;
}

async function attachListeners() {
  if (!isTauri || listenersAttached) {
    return;
  }

  listenersAttached = true;

  await listen<ServiceView>("service-status", (event) => {
    useServiceStore.setState((state) => ({
      services: upsert(state.services, event.payload),
    }));
  });

  await listen<ServiceLog>("service-log", (event) => {
    useServiceStore.setState((state) => ({
      logs: [...state.logs, event.payload].slice(-MAX_LOGS),
    }));
  });
}

async function setBusy(id: string, value: boolean) {
  useServiceStore.setState((state) => {
    const busy = { ...state.busy };
    if (value) {
      busy[id] = true;
    } else {
      delete busy[id];
    }
    return { busy };
  });
}

export const useServiceStore = create<ServiceStore>((set, get) => ({
  services: [],
  logs: [],
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
      const services = await invoke<ServiceView[]>("get_services");
      set({ services, hydrated: true });
    } catch (error) {
      set({
        hydrated: true,
        lastError: `Could not read the service list: ${String(error)}`,
      });
    }
  },

  refresh: async () => {
    if (!isTauri) {
      return;
    }

    try {
      const services = await invoke<ServiceView[]>("get_services");
      set({ services });
    } catch (error) {
      set({ lastError: `Could not refresh the service list: ${String(error)}` });
    }
  },

  create: async (input) => {
    set({ lastError: null });
    const service = await invoke<ServiceView>("create_service", { input });
    set((state) => ({ services: upsert(state.services, service) }));
    return service;
  },

  update: async (id, input) => {
    set({ lastError: null });
    const service = await invoke<ServiceView>("update_service", { id, input });
    set((state) => ({ services: upsert(state.services, service) }));
    return service;
  },

  start: async (id) => {
    set({ lastError: null });
    await setBusy(id, true);
    try {
      const service = await invoke<ServiceView>("start_service", { id });
      set((state) => ({ services: upsert(state.services, service) }));
      return service;
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
      const service = await invoke<ServiceView>("stop_service", { id });
      set((state) => ({ services: upsert(state.services, service) }));
      return service;
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
      const service = await invoke<ServiceView>("restart_service", { id });
      set((state) => ({ services: upsert(state.services, service) }));
      return service;
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
      await invoke("delete_service", { id });
      set((state) => ({
        services: state.services.filter((service) => service.id !== id),
      }));
    } catch (error) {
      set({ lastError: String(error) });
      throw error;
    } finally {
      await setBusy(id, false);
    }
  },

  clearError: () => set({ lastError: null }),
  clearLogs: () => set({ logs: [] }),
}));
