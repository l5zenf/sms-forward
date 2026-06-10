import { useState } from "react";
import {
  Cpu,
  Server,
  HardDrive,
  Highlighter,
  Plus,
  X,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { useModem } from "@/store/modem";
import { useHighlight } from "@/store/highlight";
import { PRESET_METAS } from "@/lib/highlight";
import { fmtTime } from "@/utils";

/**
 * 设置页。当前两块：
 *   - 设备信息（只读：SIM/运营商/状态更新）
 *   - 验证码高亮规则（可配置：默认规则开关 + 自定义正则增删）
 *
 * 规则一改首页立即生效（共享 useHighlight store）。
 * 持久化在 store 内部写 localStorage，这里不直接碰。
 */
export default function Settings() {
  return (
    <div className="flex flex-col gap-5">
      <HighlightSection />
      <DeviceSection />
    </div>
  );
}

/* ============================ 设备信息 ============================ */

function DeviceSection() {
  const status = useModem((s) => s.status);

  const infos: { icon: typeof Cpu; label: string; value: string }[] = [
    { icon: Cpu, label: "SIM 状态", value: status?.sim_ready ? "就绪" : "异常" },
    { icon: Server, label: "运营商", value: status?.operator ?? "—" },
    { icon: HardDrive, label: "状态更新", value: fmtTime(status?.updated_at) },
  ];

  return (
    <Card>
      <CardHeader className="border-b">
        <CardTitle>设备信息</CardTitle>
        <CardDescription>当前调制解调器只读状态</CardDescription>
      </CardHeader>
      <CardContent className="divide-y">
        {infos.map((i) => {
          const Icon = i.icon;
          return (
            <div key={i.label} className="flex items-center gap-3 py-3">
              <div className="bg-muted text-muted-foreground grid size-8 shrink-0 place-items-center rounded-md">
                <Icon className="size-4" />
              </div>
              <span className="text-muted-foreground text-sm">{i.label}</span>
              <span className="ml-auto truncate text-right font-mono text-sm">
                {i.value}
              </span>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

/* ========================= 验证码高亮规则 ========================= */

function HighlightSection() {
  const { enabledPresets, togglePreset, custom, addCustom, removeCustom } =
    useHighlight();
  const [draft, setDraft] = useState("");

  const submit = () => {
    const t = draft.trim();
    if (!t) return;
    addCustom(t);
    setDraft("");
  };

  return (
    <Card>
      <CardHeader className="border-b">
        <CardTitle className="flex items-center gap-2">
          <Highlighter className="size-4 text-primary" />
          内容高亮
        </CardTitle>
        <CardDescription>
          命中的内容会在首页短信中加粗显示。双击加粗块可快速复制。
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-5 p-4">
        {/* 预设规则：按场景勾选即可，不需要懂正则 */}
        <div>
          <div className="mb-3 flex items-center justify-between">
            <div className="text-sm font-medium">常见规则</div>
            <span className="text-muted-foreground text-xs tabular-nums">
              {enabledPresets.length} / {PRESET_METAS.length}
            </span>
          </div>
          <ul className="flex flex-col gap-1">
            {PRESET_METAS.map((p) => {
              const on = enabledPresets.includes(p.id);
              return (
                <li
                  key={p.id}
                  className={cn(
                    "hover:bg-accent/50 flex items-center gap-3 rounded-md px-3 py-2 transition-colors",
                  )}
                >
                  <button
                    type="button"
                    role="switch"
                    aria-checked={on}
                    aria-label={p.label}
                    onClick={() => togglePreset(p.id)}
                    className={cn(
                      "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
                      on
                        ? "bg-primary border-primary"
                        : "bg-muted border-input",
                    )}
                  >
                    <span
                      className={cn(
                        "inline-block size-4 transform rounded-full bg-background shadow transition-transform",
                        on ? "translate-x-4" : "translate-x-0.5",
                      )}
                    />
                  </button>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium">{p.label}</div>
                    <p className="text-muted-foreground mt-0.5 text-xs">{p.desc}</p>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>

        <Separator />

        {/* 自定义正则：预设之外的补充 */}
        <div>
          <div className="mb-2 flex items-center justify-between">
            <div className="text-sm font-medium">自定义规则</div>
            <span className="text-muted-foreground text-xs tabular-nums">
              {custom.length} 条
            </span>
          </div>
          <p className="text-muted-foreground mb-2 text-xs">
            预设没覆盖到的可自填 JavaScript 正则（无需标志位，全局匹配自动应用）。
            带捕获组时只加粗组内内容，否则整段加粗。非法正则会被忽略。
          </p>

          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              submit();
            }}
          >
            <Input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="例如：会员号[:：\s]*([A-Z]?\d{6,})"
              className="font-mono text-xs"
            />
            <Button type="submit" size="sm" variant="secondary" className="shrink-0">
              <Plus className="size-4" />
              添加
            </Button>
          </form>

          {custom.length > 0 && (
            <ul className="mt-3 flex flex-col gap-1.5">
              {custom.map((src) => (
                <li
                  key={src}
                  className="bg-muted/40 flex items-center gap-2 rounded-md px-3 py-2"
                >
                  <code className="min-w-0 flex-1 truncate font-mono text-xs">
                    {src}
                  </code>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-6 shrink-0"
                    onClick={() => removeCustom(src)}
                    aria-label={`删除规则 ${src}`}
                  >
                    <X className="size-3.5" />
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
