import { Bell, SignalHigh, Wifi, Cpu, Power } from "lucide-react";

import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { useMessages } from "@/store/messages";
import { fmtTime } from "@/utils";
import type { LucideIcon } from "lucide-react";

const EVENT_META: Record<string, { icon: LucideIcon; label: string }> = {
  started: { icon: Power, label: "启动" },
  sim_ready: { icon: Cpu, label: "SIM 就绪" },
  registered: { icon: Wifi, label: "网络注册" },
  signal: { icon: SignalHigh, label: "信号" },
  new_message: { icon: Bell, label: "新短信" },
};

function eventMeta(type: string): { icon: LucideIcon; label: string } {
  return EVENT_META[type] ?? { icon: SignalHigh, label: type };
}

function formatPayload(payload: string): string {
  try {
    const obj = JSON.parse(payload);
    // Compact, key-sorted for diffing. e.g. {"csq":24,"rssi_dbm":-65}
    const sorted: Record<string, unknown> = {};
    for (const k of Object.keys(obj).sort()) sorted[k] = obj[k];
    return JSON.stringify(sorted);
  } catch {
    return payload;
  }
}

export default function System() {
  const events = useMessages((s) => s.recentEvents);
  const loading = useMessages((s) => s.loading);

  return (
    <Card>
      <CardHeader className="border-b">
        <CardTitle>事件日志</CardTitle>
        <CardDescription>
          最近 {events.length || "30"} 条调制解调器事件（每 5 秒自动刷新）
        </CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        {loading && events.length === 0 ? (
          <ul className="divide-y">
            {Array.from({ length: 5 }).map((_, i) => (
              <li key={i} className="flex items-center gap-3 px-4 py-3">
                <Skeleton className="size-8 rounded-full" />
                <div className="flex-1 space-y-2">
                  <Skeleton className="h-3 w-1/4" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              </li>
            ))}
          </ul>
        ) : events.length === 0 ? (
          <div className="text-muted-foreground px-4 py-12 text-center text-sm">
            暂无事件
          </div>
        ) : (
          <ul className="divide-y">
            {events.map((e) => {
              const meta = eventMeta(e.event_type);
              const Icon = meta.icon;
              return (
                <li key={e.id} className="flex items-start gap-3 px-4 py-3">
                  <div className="bg-muted grid size-8 shrink-0 place-items-center rounded-full text-muted-foreground">
                    <Icon className="size-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Badge variant="outline" className="font-mono">
                        {e.event_type}
                      </Badge>
                      <span className="text-muted-foreground text-xs tabular-nums">
                        #{e.id}
                      </span>
                      <span
                        className={cn(
                          "text-muted-foreground ml-auto shrink-0 text-xs tabular-nums"
                        )}
                      >
                        {fmtTime(e.created_at)}
                      </span>
                    </div>
                    <code className="text-muted-foreground mt-1 block break-all font-mono text-xs">
                      {formatPayload(e.payload)}
                    </code>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
