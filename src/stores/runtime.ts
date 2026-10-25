import { create } from "zustand";
import { isTauri } from "@/lib/app";
import {
  getRuntimeStatus,
  repairRuntime,
  type RuntimeStatus,
} from "@/lib/runtime";

interface RuntimeStore {
  status: RuntimeStatus | null;
  hydrated: boolean;
  busy: boolean;
  error: string | null;
  hydrate: () => Promise<void>;
  repair: () => Promise<void>;
}

export const useRuntimeStore = create<RuntimeStore>((set, get) => ({
  status: null,
  hydrated: false,
  busy: false,
  error: null,

  hydrate: async () => {
    if (get().hydrated) {
      return;
    }

    if (!isTauri) {
      set({ hydrated: true });
      return;
    }

    try {
      const status = await getRuntimeStatus();
      set({ status, hydrated: true });
    } catch (error) {
      set({ hydrated: true, error: String(error) });
    }
  },

  repair: async () => {
    if (get().busy) {
      return;
    }

    set({ busy: true, error: null });
    try {
      const status = await repairRuntime();
      set({ status, hydrated: true });
    } catch (error) {
      set({ error: String(error) });
      try {
        const status = await getRuntimeStatus();
        set({ status });
      } catch {
        // Keep the original repair error.
      }
    } finally {
      set({ busy: false });
    }
  },
}));
