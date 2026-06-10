/**
 * 短信状态 → 4px 立柱色。
 *
 * 用来给首页卡片左侧加状态色条：一眼知道这条短信处于何态，
 * 没有「全部白色」的视觉单调感。
 */
export function statusAccent(status: string): string {
  switch (status) {
    case "sent":
      return "var(--success)";
    case "pending":
      return "var(--warning)";
    case "sending":
      return "var(--primary)";
    case "failed":
      return "var(--destructive)";
    case "decode_failed":
      // 解析失败用紫色，区别于普通转发失败
      return "var(--chart-4)";
    default:
      return "var(--border)";
  }
}
