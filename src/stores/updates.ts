import { create } from "zustand";
import { isTauri } from "@/lib/app";
import {
  checkForUpdate,
  closePendingUpdate,
  installPendingUpdate,
  type UpdateInfo,
} from "@/lib/updater";

interface UpdateStore {
  available: UpdateInfo | null;
  checked: boolean;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  error: string | null;
  hydrate: () => Promise<void>;
  check: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateStore>((set, get) => ({
  available: null,
  checked: false,
  checking: false,
  installing: false,
  progress: null,
  error: null,

  hydrate: async () => {
    if (get().checked) {
      return;
    }

    if (!isTauri) {
      set({ checked: true });
      return;
    }

    set({ checked: true });
    await get().check(true);
  },

  check: async (silent = false) => {
    if (get().checking || get().installing) {
      return;
    }

    set({ checking: true, error: null });
    try {
      const available = await checkForUpdate();
      set({ available, checked: true });
    } catch (error) {
      if (!silent) {
        set({ error: String(error) });
      }
    } finally {
      set({ checking: false, checked: true });
    }
  },

  install: async () => {
    if (!get().available || get().installing) {
      return;
    }

    set({ installing: true, progress: 0, error: null });
    try {
      await installPendingUpdate((update) => {
        set({ progress: update.percent });
      });
    } catch (error) {
      set({ installing: false, progress: null, error: String(error) });
    }
  },

  dismiss: () => {
    void closePendingUpdate();
    set({ available: null, progress: null, error: null });
  },
}));
