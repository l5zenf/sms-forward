// Application state for messages + dashboard. Polling lives here so the UI
// stays declarative. The modem/system pages read from src/ui/modemStore.ts.

import { create } from "zustand";
import { api } from "../api";
import type { MessagePage, ModemEventRecord, SmsMessage, StatusCounts } from "../types";

/**
 * UI 层的状态分类。底层后端只有 5 种精确状态，但 UI 上「处理中」要
 * 合并 pending + sending，「已完成」= sent，「已失败」= failed + decode_failed。
 * `all` 给首页/仪表盘只读流用——它不需要任何过滤。
 */
export type StatusFilter = "all" | "completed" | "processing" | "failed";

/**
 * UI 分类 → 后端底层 status 集合。返回 `null` 表示该分类不传 status
 * 参数（拉全部），用 `all` 取的就是这个含义。
 */
function bottomStatuses(filter: StatusFilter): string[] | null {
  switch (filter) {
    case "completed":
      return ["sent"];
    case "processing":
      return ["pending", "sending"];
    case "failed":
      return ["failed", "decode_failed"];
    case "all":
    default:
      return null; // 不发 status 参数 = 全部
  }
}

export const PAGE_SIZE = 10;
const POLL_INTERVAL_MS = 5000;

interface MessagesState {
  items: SmsMessage[];
  total: number;
  stats: StatusCounts | null;
  recentEvents: ModemEventRecord[];
  statusFilter: StatusFilter;
  query: string;
  loading: boolean;
  loadingMore: boolean;
  online: boolean;
  error: string | null;
  lastUpdated: number | null;
  autoRefresh: boolean;
  selectedId: number | null;

  setStatusFilter: (f: StatusFilter) => void;
  setQuery: (q: string) => void;
  loadMore: () => Promise<void>;
  toggleAutoRefresh: () => void;
  openDetail: (id: number | null) => void;
  refresh: () => Promise<void>;
}

let inflight: AbortController | null = null;
let inflightMore: AbortController | null = null;
let inflightEvents: AbortController | null = null;

/**
 * 把 UI 分类拆成后端 status 集，得出参数列表（多个或单个 undefined）。
 * 同一个 limit/offset/q，不同 status 字段。
 */
function buildQueryOpts(
  filter: StatusFilter,
  limit: number,
  offset: number,
  q: string | undefined,
): { limit: number; offset: number; status?: string; q?: string }[] {
  const base = { limit, offset, q };
  const statuses = bottomStatuses(filter);
  if (statuses == null) return [{ ...base }];
  return statuses.map((status) => ({ ...base, status }));
}

/**
 * 把多个并发请求的 items 合并、去重、按 id desc 排序。
 * total 用所有请求里取的最大值（每个 status 是独立子集，加总近似但后端
 * 用「各子集 total 求和」更准——这里取 max 是因为 UI 只需借此判「是否还有更多」，
 * 触底判定不能错就行）。
 */
function mergePageResult(pages: MessagePage[]): { items: SmsMessage[]; total: number } {
  if (pages.length === 0) return { items: [], total: 0 };
  const idSet = new Set<number>();
  const items: SmsMessage[] = [];
  for (const p of pages) {
    for (const m of p.items) {
      if (!idSet.has(m.id)) {
        idSet.add(m.id);
        items.push(m);
      }
    }
  }
  items.sort((a, b) => b.id - a.id);
  // total：合并多 status 的 total 求和（each 子集的 total 反映了该 status
  // 的总条数），得到的合并 total 才能正确反映 UI 分类下的总量。
  const total = pages.reduce((acc, p) => acc + p.total, 0);
  return { items, total };
}

function mergeById(prev: SmsMessage[], next: SmsMessage[]): SmsMessage[] {
  if (next.length === 0) return prev;
  const seen = new Set(prev.map((m) => m.id));
  const fresh = next.filter((m) => !seen.has(m.id));
  return [...fresh, ...prev].sort((a, b) => b.id - a.id);
}

function dedupeAppend(prev: SmsMessage[], next: SmsMessage[]): SmsMessage[] {
  const seen = new Set(prev.map((m) => m.id));
  const fresh = next.filter((m) => !seen.has(m.id));
  return [...prev, ...fresh].sort((a, b) => b.id - a.id);
}

export const useMessages = create<MessagesState>((set, get) => ({
  items: [],
  total: 0,
  stats: null,
  recentEvents: [],
  statusFilter: "all",
  query: "",
  loading: false,
  loadingMore: false,
  online: true,
  error: null,
  lastUpdated: null,
  autoRefresh: true,
  selectedId: null,

  setStatusFilter: (f) => {
    set({ statusFilter: f, items: [], total: 0 });
    void get().refresh();
  },
  setQuery: (q) => {
    set({ query: q, items: [], total: 0 });
    void get().refresh();
  },
  loadMore: async () => {
    const s = get();
    if (s.loadingMore || s.items.length >= s.total) return;
    inflightMore?.abort();
    const ctrl = new AbortController();
    inflightMore = ctrl;
    const { statusFilter, query, items } = s;
    set({ loadingMore: true });
    try {
      const optsList = buildQueryOpts(
        statusFilter,
        PAGE_SIZE,
        items.length,
        query.trim() || undefined,
      );
      const pages = await Promise.all(
        optsList.map((o) => api.messages(o, ctrl.signal)),
      );
      if (ctrl.signal.aborted) return;
      const merged = mergePageResult(pages);
      set((st) => ({
        items: dedupeAppend(st.items, merged.items),
        total: Math.max(st.total, merged.total),
        loadingMore: false,
        online: true,
        error: null,
      }));
    } catch (e) {
      if ((e as Error).name === "AbortError") return;
      set({
        loadingMore: false,
        online: false,
        error: e instanceof Error ? e.message : "加载更多失败",
      });
    } finally {
      if (inflightMore === ctrl) inflightMore = null;
    }
  },
  toggleAutoRefresh: () => {
    set({ autoRefresh: !get().autoRefresh });
  },
  openDetail: (id) => set({ selectedId: id }),

  refresh: async () => {
    inflight?.abort();
    const ctrl = new AbortController();
    inflight = ctrl;
    const { statusFilter, query, items } = get();
    const limit = Math.max(PAGE_SIZE, items.length);
    set({ loading: items.length === 0 });
    try {
      const optsList = buildQueryOpts(statusFilter, limit, 0, query.trim() || undefined);
      const [pages, stats] = await Promise.all([
        Promise.all(optsList.map((o) => api.messages(o, ctrl.signal))),
        api.stats(ctrl.signal),
      ]);
      if (ctrl.signal.aborted) return;
      const merged = mergePageResult(pages);
      set((st) => ({
        items: mergeById(st.items, merged.items).slice(
          0,
          Math.max(st.items.length, merged.items.length),
        ),
        // total：refresh 时直接覆盖——新总集可能更大；store 取 max 防抖闪烁
        total: Math.max(st.total, merged.total),
        stats,
        loading: false,
        online: true,
        error: null,
        lastUpdated: Date.now(),
      }));
    } catch (e) {
      if ((e as Error).name === "AbortError") return;
      set({
        loading: false,
        online: false,
        error: e instanceof Error ? e.message : "刷新失败",
      });
    } finally {
      if (inflight === ctrl) inflight = null;
    }
  },
}));

// Polling: drives both the message list and the recent-events feed.
if (typeof window !== "undefined") {
  const tick = async () => {
    const s = useMessages.getState();
    if (!s.autoRefresh) return;
    void s.refresh();
    inflightEvents?.abort();
    const ctrl = new AbortController();
    inflightEvents = ctrl;
    try {
      const events = await api.modemEvents(15, ctrl.signal);
      if (ctrl.signal.aborted) return;
      useMessages.setState({ recentEvents: events });
    } catch {
      /* swallowed */
    } finally {
      if (inflightEvents === ctrl) inflightEvents = null;
    }
  };
  void tick();
  window.setInterval(tick, POLL_INTERVAL_MS);
}
