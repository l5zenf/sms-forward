/**
 * 基于字符串生成稳定的颜色对（背景 + 前景）。
 *
 * 用途：给发件人头像配色——同一个发件人永远是同一种颜色，
 * 不同发件人视觉可区分，帮助用户在长列表里形成「这是谁」的视觉记忆。
 *
 * 实现：简单 djb2 hash → hue；用 HSL 然后 CSS variable 输出，
 * 调用方直接 inline style 应用即可，不依赖 Tailwind 任意类生成。
 */

// 16 色手调调色板，比 8 色显著降低单一列表里多 sender 的撞色概率。
// 全部落在「明度高 + 中低饱和」区间，浅色背景上不刺眼但各有辨识度。
const PALETTE: { bg: string; fg: string }[] = [
  { bg: "oklch(0.92 0.05 250)", fg: "oklch(0.40 0.13 250)" }, // 蓝
  { bg: "oklch(0.92 0.06 30)",  fg: "oklch(0.42 0.14 30)" },  // 橙
  { bg: "oklch(0.90 0.07 150)", fg: "oklch(0.40 0.13 150)" }, // 绿
  { bg: "oklch(0.92 0.05 320)", fg: "oklch(0.45 0.13 320)" }, // 紫
  { bg: "oklch(0.92 0.07 0)",   fg: "oklch(0.42 0.13 0)" },   // 红
  { bg: "oklch(0.92 0.06 90)",  fg: "oklch(0.42 0.13 90)" },  // 黄绿
  { bg: "oklch(0.90 0.05 200)", fg: "oklch(0.40 0.11 200)" }, // 青
  { bg: "oklch(0.92 0.06 280)", fg: "oklch(0.45 0.13 280)" }, // 蓝紫
  { bg: "oklch(0.92 0.05 60)",  fg: "oklch(0.42 0.13 60)" },  // 金黄
  { bg: "oklch(0.90 0.06 170)", fg: "oklch(0.40 0.12 170)" }, // 翠绿
  { bg: "oklch(0.92 0.06 340)", fg: "oklch(0.45 0.13 340)" }, // 玫红
  { bg: "oklch(0.90 0.05 230)", fg: "oklch(0.40 0.11 230)" }, // 钢蓝
  { bg: "oklch(0.92 0.06 130)", fg: "oklch(0.40 0.12 130)" }, // 草绿
  { bg: "oklch(0.90 0.06 10)",  fg: "oklch(0.42 0.12 10)" },  // 砖红
  { bg: "oklch(0.92 0.05 300)", fg: "oklch(0.45 0.13 300)" }, // 品紫
  { bg: "oklch(0.90 0.05 190)", fg: "oklch(0.40 0.11 190)" }, // 蓝绿
];

function hash(str: string): number {
  let h = 5381;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) + h) ^ str.charCodeAt(i);
  }
  return Math.abs(h);
}

/** 给定任意 key（通常是 sender），返回稳定的颜色对。 */
export function avatarColor(key: string | null | undefined): { bg: string; fg: string } {
  if (!key) return PALETTE[0];
  return PALETTE[hash(key) % PALETTE.length];
}
