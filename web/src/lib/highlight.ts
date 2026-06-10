/**
 * 短信内容高亮引擎。
 *
 * 模型：
 *   - PRESETS：内置「场景规则集合」，每条是一个或多个正则 + 人话描述，
 *     用户在设置页按场景开关即可，不需要懂正则。
 *   - custom：用户自写正则源码，作为预设之外的补充。
 *
 * 取舍：
 *   - 强调「捕获组」内的内容（如「验证码：392014」只加粗 392014，
 *     「关键词」不加粗），避免一行全是粗体。
 *   - 区间合并防止相邻匹配嵌套。
 *   - localStorage 容错：脏数据/旧版字段都会被迁移或回退。
 */

/** 一段文本：要么普通字面量，要么需要强调。 */
export interface Segment {
  text: string;
  emph: boolean;
}

/** 一条预设规则的元信息（设置页展示用）。 */
export interface PresetMeta {
  id: string;
  /** 下拉/勾选项标签。 */
  label: string;
  /** 一句话说明覆盖什么。 */
  desc: string;
}

/** 预设规则的完整定义（含正则，运行期用）。 */
interface Preset extends PresetMeta {
  patterns: RegExp[];
}

/** 用户可配置的高亮规则集。 */
export interface HighlightConfig {
  /** 已启用的预设规则 id 列表。 */
  enabledPresets: string[];
  /** 用户自定义正则源码字符串列表。非法正则加载时静默丢弃。 */
  custom: string[];
}

const STORAGE_KEY = "gg-guard.highlight.config";

/**
 * 内置预设规则集合。覆盖中国短信里用户真正要「照抄」的几类信息。
 *
 * 注：所有数字类规则都用「关键词/上下文 + 数字」的组合匹配，避免单独
 * 匹配所有数字（会把金额、流水号也误加粗）。每条首选带捕获组，只强调
 * 那 1 段真正有用的内容。
 */
export const PRESETS: Preset[] = [
  {
    id: "verification",
    label: "验证码 / 动态码",
    desc: "验证码、校验码、动态码、code、otp 等关键词 + 3-8 位数字",
    patterns: [
      /(?:验证码|校验码|动态码|动态密码|验证序列号|序列号)[:：\s]*([0-9](?:[\s-]?[0-9]){2,7})/g,
      /(?:code|captcha|otp|verification|password)[:\s\S]{0,15}?([0-9](?:[\s-]?[0-9]){2,7})/gi,
    ],
  },
  {
    id: "pickup",
    label: "取件码 / 快递柜",
    desc: "取件码、取货码、快递柜编号（如 8-1-101 / A1234）",
    patterns: [
      /(?:取件码|取货码|取衣柜码|取餐码)[:：\s]*([0-9A-Za-z][\s-]?[0-9A-Za-z\-]{2,11})/g,
      /(?:快递柜|丰巢|菜鸟驿站)[:：\s]*([0-9A-Za-z][\s\-]?[0-9A-Za-z\-]{1,11})/g,
    ],
  },
  {
    id: "amount",
    label: "金额",
    desc: "人民币金额，如 528.00 元、¥123.45",
    patterns: [
      /(?:消费|金额|金额为|合计|合计金额|扣款|入账)[:：\s]*([0-9][0-9,]*\.?[0-9]{0,2}\s*(?:元|圆|RMB|CNY))/g,
      /(¥|￥)\s*([0-9][0-9,]*\.?[0-9]{0,2})/g,
    ],
  },
  {
    id: "card",
    label: "银行卡尾号",
    desc: "尾号 XXXX 的卡 / 账号末四位",
    patterns: [/尾号\s*([0-9]{4})/g],
  },
  {
    id: "trip",
    label: "班次号",
    desc: "高铁车次 G/D/C+数字、航班 CA/MU/CZ+数字、座位号",
    patterns: [
      /(?:车次|航班号|航班)\s*([GCZD]\d{2,4})/g,
      /航班\s*([A-Z]{2}\d{2,4})/g,
      /座位号[:：\s]*([0-9]{1,3}[A-F])/g,
    ],
  },
  {
    id: "link",
    label: "网址 / 短链",
    desc: "短信里的 http(s) 链接、短链",
    patterns: [/(https?:\/\/[^\s，。、；""''\u3000)]+)/g],
  },
];

/** 暴露给设置页的预设元信息（不含正则，避免泄露实现细节）。 */
export const PRESET_METAS: PresetMeta[] = PRESETS.map(({ id, label, desc }) => ({
  id,
  label,
  desc,
}));

/** 默认配置：开「验证码」一条（开箱即用最常见的场景）。 */
const DEFAULT_CONFIG: HighlightConfig = {
  enabledPresets: ["verification"],
  custom: [],
};

/**
 * 读取用户配置并迁移旧版字段。
 *
 * v1 用 `defaultsEnabled: boolean` 控制验证码；这里把它迁移成
 * `enabledPresets` membership in `verification`。
 */
export function loadConfig(): HighlightConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_CONFIG, enabledPresets: [...DEFAULT_CONFIG.enabledPresets] };
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    // 迁移：旧版 defaultsEnabled
    if ("defaultsEnabled" in parsed && !("enabledPresets" in parsed)) {
      const v = parsed.defaultsEnabled === true ? ["verification"] : [];
      return { enabledPresets: v, custom: [] };
    }
    const enabledRaw = parsed.enabledPresets;
    const enabled = Array.isArray(enabledRaw)
      ? enabledRaw.filter((x): x is string => typeof x === "string")
      : [...DEFAULT_CONFIG.enabledPresets];
    const custom = Array.isArray(parsed.custom)
      ? parsed.custom.filter((x): x is string => typeof x === "string")
      : [];
    return { enabledPresets: enabled, custom };
  } catch {
    return { ...DEFAULT_CONFIG, enabledPresets: [...DEFAULT_CONFIG.enabledPresets] };
  }
}

export function saveConfig(c: HighlightConfig): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
  } catch {
    /* ignore quota / unavailable */
  }
}

/** 把用户输入的正则源码安全编译成 RegExp（非法返回 null）。 */
function compileCustom(src: string): RegExp | null {
  try {
    return new RegExp(src, "g");
  } catch {
    return null;
  }
}

/** 按 config 收集实际生效的 RegExp 集合。 */
function activePatterns(config: HighlightConfig): RegExp[] {
  const out: RegExp[] = [];
  for (const p of PRESETS) {
    if (config.enabledPresets.includes(p.id)) {
      for (const re of p.patterns) out.push(new RegExp(re.source, re.flags));
    }
  }
  for (const src of config.custom) {
    const re = compileCustom(src);
    if (re) out.push(re);
  }
  return out;
}

/**
 * 把任意文本切成带 emph 标记的段落。仅强调「捕获组」内的内容，
 * 而不是关键词。若无捕获组则强调整段匹配。
 */
export function highlight(text: string, config: HighlightConfig): Segment[] {
  if (!text) return [{ text: text ?? "", emph: false }];

  const patterns = activePatterns(config);
  if (patterns.length === 0) return [{ text, emph: false }];

  type Range = { start: number; end: number };
  const ranges: Range[] = [];
  for (const re of patterns) {
    re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      const group = m[1] ?? m[2];
      const start = m.index + (group != null ? m[0].indexOf(group) : 0);
      const end = start + (group != null ? group.length : m[0].length);
      if (end > start) ranges.push({ start, end });
      if (m.index === re.lastIndex) re.lastIndex++;
    }
  }
  if (ranges.length === 0) return [{ text, emph: false }];

  ranges.sort((a, b) => a.start - b.start);
  const merged: Range[] = [];
  for (const r of ranges) {
    const last = merged[merged.length - 1];
    if (last && r.start <= last.end) last.end = Math.max(last.end, r.end);
    else merged.push({ ...r });
  }

  const segs: Segment[] = [];
  let cursor = 0;
  for (const r of merged) {
    if (r.start > cursor) segs.push({ text: text.slice(cursor, r.start), emph: false });
    segs.push({ text: text.slice(r.start, r.end), emph: true });
    cursor = r.end;
  }
  if (cursor < text.length) segs.push({ text: text.slice(cursor), emph: false });
  return segs;
}
