// Presentation helpers shared across pages. UI-agnostic; no icon imports so
// pages can pick the lucide icon they want at render time.
import type { VariantProps } from "class-variance-authority";
import { badgeVariants } from "@/components/ui/badge";

export function fmtTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

export function timeAgo(ts: number | null): string {
  if (ts == null) return "—";
  const diff = Math.max(0, Date.now() - ts);
  const s = Math.floor(diff / 1000);
  if (s < 5) return "刚刚";
  if (s < 60) return `${s} 秒前`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} 分钟前`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} 小时前`;
  return `${Math.floor(h / 24)} 天前`;
}

export function fmtAgo(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso).getTime();
  if (Number.isNaN(d)) return iso;
  return timeAgo(d);
}

const STATUS_LABEL: Record<string, string> = {
  pending: "待转发",
  sending: "转发中",
  sent: "已送达",
  failed: "已失败",
  decode_failed: "解析失败",
};

export function statusLabel(status: string): string {
  return STATUS_LABEL[status] ?? status;
}

export type BadgeVariant = VariantProps<typeof badgeVariants>["variant"];

/** Map an SMS status to a shadcn Badge variant for consistent coloring. */
export function statusBadgeVariant(status: string): BadgeVariant {
  switch (status) {
    case "sent":
      return "success";
    case "sending":
      return "secondary";
    case "pending":
      return "outline";
    case "failed":
      return "destructive";
    case "decode_failed":
      return "warning";
    default:
      return "outline";
  }
}

/** Map CSQ (0..31 per GSM 05.08) to a human label. */
export function csqLabel(csq: number | null): string {
  if (csq == null) return "—";
  if (csq >= 28) return "极好";
  if (csq >= 20) return "良好";
  if (csq >= 10) return "一般";
  if (csq > 0) return "较差";
  return "无信号";
}

export function csqRatio(csq: number | null): number {
  if (csq == null) return 0;
  return Math.max(0, Math.min(1, csq / 31));
}

export function preview(content: string | null, max = 70): string {
  if (content == null) return "(内容为空)";
  const single = content.replace(/\s+/g, " ").trim();
  return single.length > max ? `${single.slice(0, max)}…` : single;
}

export function initials(sender: string | null): string {
  if (!sender) return "?";
  const t = sender.trim();
  return t.length <= 2 ? t : t.slice(-2);
}
