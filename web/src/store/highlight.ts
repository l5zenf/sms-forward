/**
 * 高亮配置 store。让设置页改规则后首页即时重渲染，而不是各自读 localStorage。
 */
import { create } from "zustand";
import { useShallow } from "zustand/shallow";
import { loadConfig, saveConfig, type HighlightConfig } from "@/lib/highlight";

interface HighlightStore extends HighlightConfig {
  /** 切换某个预设规则的启用状态。 */
  togglePreset: (id: string) => void;
  addCustom: (src: string) => void;
  removeCustom: (src: string) => void;
}

function persist(get: () => HighlightStore): void {
  saveConfig({
    enabledPresets: [...get().enabledPresets],
    custom: [...get().custom],
  });
}

export const useHighlight = create<HighlightStore>((set, get) => ({
  ...loadConfig(),
  togglePreset: (id) => {
    const cur = get().enabledPresets;
    const next = cur.includes(id)
      ? cur.filter((x) => x !== id)
      : [...cur, id];
    set({ enabledPresets: next });
    persist(get);
  },
  addCustom: (src) => {
    const t = src.trim();
    if (!t || get().custom.includes(t)) return;
    set({ custom: [...get().custom, t] });
    persist(get);
  },
  removeCustom: (src) => {
    set({ custom: get().custom.filter((c) => c !== src) });
    persist(get);
  },
}));

/**
 * 便捷 selector：返回纯 HighlightConfig（不含方法）。
 * 给只需要读配置的消费者用，避免把整个 store 传进低层组件造成类型/重渲染噪声。
 */
export function useHighlightConfig(): HighlightConfig {
  return useHighlight(
    useShallow((s) => ({
      enabledPresets: s.enabledPresets,
      custom: s.custom,
    })),
  );
}
