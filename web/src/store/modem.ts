// Modem status store, used by the Modem page and the Topbar indicator.

import { create } from "zustand";
import { api } from "../api";
import type { ModemStatusRecord } from "../types";

interface ModemState {
  status: ModemStatusRecord | null;
  online: boolean;
  loading: boolean;
  autoRefresh: boolean;
  refresh: () => Promise<void>;
  toggleAutoRefresh: () => void;
}

const POLL_INTERVAL_MS = 5000;
let inflight: AbortController | null = null;

export const useModem = create<ModemState>((set, get) => ({
  status: null,
  online: true,
  loading: false,
  autoRefresh: true,
  refresh: async () => {
    inflight?.abort();
    const ctrl = new AbortController();
    inflight = ctrl;
    set({ loading: !get().status });
    try {
      const status = await api.modemStatus(ctrl.signal);
      if (ctrl.signal.aborted) return;
      set({ status, online: true, loading: false });
    } catch (e) {
      if ((e as Error).name === "AbortError") return;
      set({ online: false, loading: false });
    } finally {
      if (inflight === ctrl) inflight = null;
    }
  },
  toggleAutoRefresh: () => set({ autoRefresh: !get().autoRefresh }),
}));

if (typeof window !== "undefined") {
  const tick = () => {
    if (useModem.getState().autoRefresh) void useModem.getState().refresh();
  };
  void tick();
  window.setInterval(tick, POLL_INTERVAL_MS);
}
