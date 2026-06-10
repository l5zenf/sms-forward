import { useState } from "react";

import { cn } from "@/lib/utils";
import { highlight, type HighlightConfig } from "@/lib/highlight";
import { useHighlightConfig } from "@/store/highlight";
import { toast } from "@/store/toast";

/**
 * 短信正文渲染：全文铺开 + 命中规则的片段加粗（验证码等），双击加粗块可复制。
 *
 * 首页短行和信息页就地展开的详情，正文呈现要求一致：全文、不截断、验证码加粗、
 * 可点复制。所以抽出来共享，避免两边各渲染一套出现风格漂移。
 *
 * select-none：双击复制时不留选区高亮，反馈交给 toast 处理。
 */
export function SmsContent({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  const cfg = useHighlightConfig();
  return <SmsContentImpl text={text} cfg={cfg} className={className} />;
}

// 内部组件：把 cfg 作为 prop 暴露出来，方便不需要订阅 store 的调用方
// （目前仅 SmsContent 自己用，但保留了灵活性）。
function SmsContentImpl({
  text,
  cfg,
  className,
}: {
  text: string;
  cfg: HighlightConfig;
  className?: string;
}) {
  const segs = highlight(text, cfg);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);

  const copy = async (i: number, content: string) => {
    let ok = false;
    try {
      await navigator.clipboard.writeText(content);
      ok = true;
    } catch {
      try {
        const ta = document.createElement("textarea");
        ta.value = content;
        ta.style.position = "fixed";
        ta.style.opacity = "0";
        document.body.appendChild(ta);
        ta.select();
        ok = document.execCommand("copy");
        ta.remove();
      } catch {
        ok = false;
      }
    }
    if (ok) {
      setCopiedIdx(i);
      window.setTimeout(() => setCopiedIdx((cur) => (cur === i ? null : cur)), 1200);
      toast(`已复制 ${content}`);
    }
  };

  return (
    <p
      className={cn(
        "select-none text-sm leading-relaxed whitespace-pre-wrap break-words",
        className,
      )}
    >
      {segs.map((s, i) =>
        s.emph ? (
          <strong
            key={i}
            title="双击复制"
            onDoubleClick={() => void copy(i, s.text)}
            className={cn(
              // 「荧光笔」感：高饱和黄底 + 深色字，对比强烈，一眼锁定要抄的数字
              "font-bold tracking-wide rounded px-1 -mx-0.5 tabular-nums cursor-pointer transition-colors break-all",
              "bg-[oklch(0.92_0.18_95)] text-[oklch(0.32_0.12_75)]",
              "hover:bg-[oklch(0.88_0.19_95)]",
              // 复制成功片刻切到绿色作为反馈
              copiedIdx === i && "bg-[var(--success)]/20 text-[var(--success)]",
            )}
          >
            {s.text}
          </strong>
        ) : (
          <span key={i}>{s.text}</span>
        ),
      )}
    </p>
  );
}
