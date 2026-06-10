/**
 * 极简 toast store。无需第三方依赖，自管队列即可。
 *
 * 设计：固定 push/dismiss + 自动过期，足以覆盖「复制成功」这类瞬时反馈。
 * 真要复杂（action stack/持久）再换 sonner，目前不划算。
 */
import { create } from "zustand";

export interface ToastItem {
  id: number;
  message: string;
}

interface ToastState {
  items: ToastItem[];
  /** 推一条；同一 message 会去重避免连击刷屏。自动 3s 后消失。 */
  push: (message: string) => void;
  dismiss: (id: number) => void;
}

let seq = 1;

export const useToast = create<ToastState>((set, get) => ({
  items: [],
  push: (message) => {
    // 同文去重：避免对同一验证码连击刷出一片
    const existing = get().items.find((t) => t.message === message);
    if (existing) return;
    const id = seq++;
    set((s) => ({ items: [...s.items, { id, message }] }));
    window.setTimeout(() => get().dismiss(id), 3000);
  },
  dismiss: (id) => set((s) => ({ items: s.items.filter((t) => t.id !== id) })),
}));

/** 便捷命令式 API（不订阅 store 的消费者也能用）。 */
export function toast(message: string): void {
  useToast.getState().push(message);
}
