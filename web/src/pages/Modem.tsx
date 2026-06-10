import { Wifi, Check, X, SignalHigh, Cpu } from "lucide-react";

import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useModem } from "@/store/modem";
import { csqLabel, csqRatio, fmtTime } from "@/utils";

export default function Modem() {
  const status = useModem((s) => s.status);
  const loading = useModem((s) => s.loading);

  if (loading && !status) {
    return (
      <div className="grid gap-4 md:grid-cols-3">
        {Array.from({ length: 3 }).map((_, i) => (
          <Card key={i}>
            <CardHeader>
              <Skeleton className="h-4 w-1/3" />
            </CardHeader>
            <CardContent className="space-y-3">
              <Skeleton className="h-16 w-full" />
              <Skeleton className="h-4 w-2/3" />
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }

  if (!status) {
    return (
      <Card>
        <CardContent className="text-muted-foreground py-12 text-center text-sm">
          暂无调制解调器数据
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      {/* State tiles */}
      <div className="grid gap-4 md:grid-cols-3">
        <StateTile
          icon={<Cpu className="size-5" />}
          label="SIM 卡"
          value={status.sim_ready ? "就绪" : "异常"}
          ok={status.sim_ready}
        />
        <StateTile
          icon={<Wifi className="size-5" />}
          label="网络注册"
          value={status.registered ? "已注册" : "未注册"}
          ok={status.registered}
        />
        <StateTile
          icon={<SignalHigh className="size-5" />}
          label="漫 游"
          value={status.roaming ? "是" : "否"}
          ok={!status.roaming}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Signal meter */}
        <Card className="lg:col-span-2">
          <CardHeader className="border-b">
            <CardTitle>信号质量</CardTitle>
            <CardDescription>CSQ 与 RSSI</CardDescription>
          </CardHeader>
          <CardContent className="space-y-5 py-6">
            <div className="flex items-end justify-between">
              <div>
                <div className="text-5xl font-semibold tabular-nums">
                  {status.csq ?? "—"}
                </div>
                <div className="text-muted-foreground mt-1 text-sm">CSQ</div>
              </div>
              <div className="text-right">
                <div className="text-2xl font-medium">{csqLabel(status.csq)}</div>
                <div className="text-muted-foreground mt-1 text-sm tabular-nums">
                  {status.rssi_dbm ? `${status.rssi_dbm} dBm` : "—"}
                </div>
              </div>
            </div>

            {/* Segmented bar */}
            <div className="flex items-end gap-1.5">
              {Array.from({ length: 20 }).map((_, i) => {
                const filled = i < Math.round(csqRatio(status.csq) * 20);
                const hue =
                  i < 6
                    ? "bg-destructive"
                    : i < 12
                      ? "bg-[var(--warning)]"
                      : "bg-[var(--success)]";
                return (
                  <div
                    key={i}
                    className={cn(
                      "flex-1 rounded-sm transition-colors",
                      filled
                        ? hue
                        : "bg-muted"
                    )}
                    style={{ height: `${12 + i * 3}px` }}
                  />
                );
              })}
            </div>

            <div className="text-muted-foreground flex justify-between text-xs">
              <span>0</span>
              <span>15</span>
              <span>31</span>
            </div>
          </CardContent>
        </Card>

        {/* Info grid */}
        <Card>
          <CardHeader className="border-b">
            <CardTitle>运营商与错误</CardTitle>
            <CardDescription>归属网络与最近一次错误</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 py-4 text-sm">
            <InfoRow label="运营商" value={status.operator ?? "—"} />
            <InfoRow label="最近更新" value={fmtTime(status.updated_at)} />
            <InfoRow label="CSQ" value={String(status.csq ?? "—")} />
            <InfoRow label="RSSI" value={status.rssi_dbm ? `${status.rssi_dbm} dBm` : "—"} />
            <InfoRow
              label="最近错误"
              value={status.last_error ?? "无错误"}
              danger={!!status.last_error}
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function StateTile({
  icon,
  label,
  value,
  ok,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  ok: boolean;
}) {
  return (
    <Card className="gap-0 py-0">
      <CardContent className="flex items-center gap-4 p-5">
        <div
          className={cn(
            "grid size-12 place-items-center rounded-xl",
            ok
              ? "bg-[color-mix(in_oklch,var(--success)_18%,transparent)] text-[var(--success)]"
              : "bg-destructive/15 text-destructive"
          )}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-muted-foreground text-xs">{label}</div>
          <div className="mt-0.5 flex items-center gap-1.5 text-lg font-semibold">
            {ok ? (
              <Check className="size-4 text-[var(--success)]" />
            ) : (
              <X className="size-4 text-destructive" />
            )}
            {value}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function InfoRow({
  label,
  value,
  danger,
}: {
  label: string;
  value: string;
  danger?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-muted-foreground shrink-0">{label}</span>
      <span
        className={cn(
          "truncate text-right font-medium",
          danger && "text-destructive"
        )}
      >
        {value}
      </span>
    </div>
  );
}
