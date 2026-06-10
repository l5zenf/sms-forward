import { useToast } from "@/store/toast";
import { Check } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Toast 渲染容器。固定在顶部居中，订阅全局 toast store。
 * 单页应用内容长滚动时也能保持可见（脱离 main overflow）。
 */
export function Toaster() {
  const items = useToast((s) => s.items);
  const dismiss = useToast((s) => s.dismiss);

  return (
    <div className="pointer-events-none fixed top-20 left-1/2 z-[100] flex -translate-x-1/2 flex-col items-center gap-2">
      {items.map((t) => (
        <button
          key={t.id}
          type="button"
          onClick={() => dismiss(t.id)}
          className={cn(
            "bg-card text-card-foreground pointer-events-auto flex items-center gap-2 rounded-lg border px-3 py-2 text-sm shadow-lg",
            "animate-in fade-in slide-in-from-top-2 duration-200",
          )}
        >
          <span className="bg-[var(--success)]/15 text-[var(--success)] grid size-5 shrink-0 place-items-center rounded-full">
            <Check className="size-3" />
          </span>
          <span className="font-medium">{t.message}</span>
        </button>
      ))}
    </div>
  );
}
