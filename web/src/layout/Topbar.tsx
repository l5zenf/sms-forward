import { useEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import { RefreshCw, AlertTriangle, Check, Wifi } from "lucide-react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { useModem } from "@/store/modem";
import { useMessages } from "@/store/messages";
import { timeAgo } from "@/utils";
import { usePageMeta } from "./Sidebar";

export function HealthPill() {
  const modem = useModem((s) => s.status);
  const online = useMessages((s) => s.online);
  const lastUpdated = useMessages((s) => s.lastUpdated);
  const location = useLocation();
  const [open, setOpen] = useState(false);

  // Auto-dismiss the popover when the route changes (otherwise it lingers as
  // the user navigates via the sidebar while inspecting status).
  useEffect(() => {
    setOpen(false);
  }, [location.pathname]);

  const simOk = modem?.sim_ready ?? false;
  const regOk = modem?.registered ?? false;

  const tone = !online ? "bad" : !simOk || !regOk ? "warn" : "ok";
  const label = !online
    ? "离线"
    : tone === "warn"
      ? "部分异常"
      : "运行正常";

  const rows = [
    {
      k: "服务在线",
      ok: online,
      okText: "在线",
      badText: "离线",
    },
    {
      k: "SIM 卡",
      ok: simOk,
      okText: "就绪",
      badText: modem?.last_error || "异常",
    },
    {
      k: "网络注册",
      ok: regOk,
      okText: "已注册",
      badText: "未注册",
    },
  ];

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            "hover:bg-accent flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition-colors cursor-pointer",
            tone === "ok" &&
              "border-[color-mix(in_oklch,var(--success)_45%,transparent)] text-[var(--success)]",
            tone === "warn" &&
              "border-[color-mix(in_oklch,var(--warning)_50%,transparent)] text-[var(--warning)]",
            tone === "bad" &&
              "border-destructive/40 text-destructive"
          )}
        >
          <span
            className={cn(
              "size-2 rounded-full",
              tone === "ok" && "bg-[var(--success)]",
              tone === "warn" && "bg-[var(--warning)]",
              tone === "bad" && "bg-destructive"
            )}
          />
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-64 p-0">
        <div className="flex items-center justify-between border-b px-4 py-3">
          <span className="text-sm font-semibold">系统健康</span>
          <span
            className={cn(
              "text-xs",
              tone === "ok" && "text-[var(--success)]",
              tone === "warn" && "text-[var(--warning)]",
              tone === "bad" && "text-destructive"
            )}
          >
            {label}
          </span>
        </div>
        <ul className="flex flex-col gap-2.5 px-4 py-3">
          {rows.map((r) => (
            <li key={r.k} className="flex items-center gap-2.5 text-sm">
              <span
                className={cn(
                  "grid size-5 place-items-center rounded-full",
                  r.ok
                    ? "bg-[color-mix(in_oklch,var(--success)_22%,transparent)] text-[var(--success)]"
                    : "bg-destructive/15 text-destructive"
                )}
              >
                {r.ok ? <Check className="size-3" /> : <AlertTriangle className="size-3" />}
              </span>
              <span className="text-muted-foreground">{r.k}</span>
              <span
                className={cn(
                  "ml-auto font-medium tabular-nums",
                  r.ok
                    ? "text-[var(--success)]"
                    : "text-destructive"
                )}
              >
                {r.ok ? r.okText : r.badText}
              </span>
            </li>
          ))}
        </ul>
        <div className="text-muted-foreground flex items-center gap-1.5 border-t px-4 py-2 text-xs tabular-nums">
          <Wifi className="size-3" />
          更新于 {timeAgo(lastUpdated)}
        </div>
      </PopoverContent>
    </Popover>
  );
}

export function Topbar({ onRefresh }: { onRefresh: () => void }) {
  const meta = usePageMeta();
  const autoRefresh = useMessages((s) => s.autoRefresh);
  const toggleAutoRefresh = useMessages((s) => s.toggleAutoRefresh);
  const online = useMessages((s) => s.online);

  return (
    <header className="supports-[backdrop-filter]:bg-background/60 sticky top-0 z-30 flex h-16 items-center justify-between gap-4 border-b px-4 backdrop-blur-md md:px-6">
      <div className="min-w-0">
        <h1 className="truncate text-base font-semibold leading-tight">
          {meta.title}
        </h1>
        <p className="text-muted-foreground truncate text-xs">{meta.sub}</p>
      </div>

      <div className="flex items-center gap-2">
        <HealthPill />

        <Button
          variant={autoRefresh ? "secondary" : "outline"}
          size="sm"
          onClick={toggleAutoRefresh}
          aria-pressed={autoRefresh}
          title={autoRefresh ? "自动刷新中（每 5 秒）" : "已暂停"}
        >
          <RefreshCw
            className={cn("size-3.5", autoRefresh && "animate-spin")}
            style={autoRefresh ? { animationDuration: "3s" } : undefined}
          />
          <span className="hidden sm:inline">
            {autoRefresh ? "自动" : "暂停"}
          </span>
        </Button>

        <Button
          variant="outline"
          size="icon"
          onClick={onRefresh}
          title="立即刷新"
          aria-label="立即刷新"
          disabled={!online}
        >
          <RefreshCw className="size-4" />
        </Button>
      </div>
    </header>
  );
}
