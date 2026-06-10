import { useEffect } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  SignalHigh,
  Activity,
  Cpu,
  Wifi,
  Bell,
  Power,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Skeleton } from "@/components/ui/skeleton";
import { SmsContent } from "@/components/sms-content";
import { avatarColor } from "@/lib/avatar-color";
import { statusAccent } from "@/lib/status-accent";
import { useMessages } from "@/store/messages";
import { useModem } from "@/store/modem";
import {
  csqLabel,
  csqRatio,
  fmtAgo,
  fmtTime,
  initials,
} from "@/utils";
import type { SmsMessage, ModemEventRecord } from "@/types";

/**
 * 首页 = 你打开这个应用唯一会盯的东西：「我刚收到的短信」。
 *
 * 布局取舍：
 *   - 短信列表是绝对主体，占据完整宽度，因为这是用户唯一真正在意的。
 *   - 信号、事件这些设备侧信息是辅助证据（证明设备还活着），收在右侧
 *     窄列，权重明显低于短信。
 *   - 转发失败（webhook 投递失败）对用户不是重点：只在短信行右下角放
 *     一个色点，hover 显示原因。不再用红色状态条 / 异常队列这种强 UI。
 */
export default function Dashboard() {
  const items = useMessages((s) => s.items);
  const loading = useMessages((s) => s.loading);
  const events = useMessages((s) => s.recentEvents);

  // 首页只看「最近所有状态」的混合流。如果用户刚从信息页（带着某个 UI 分类
  // 过滤状态）跳回首页，会把那部分过滤过的数据带过来——这里强制切回 all，
  // 保证首页始终是混合流（首页 7 张不够 view all 也就丢了首页价值）。
  useEffect(() => {
    if (useMessages.getState().statusFilter !== "all") {
      useMessages.getState().setStatusFilter("all");
    }
  }, []);

  return (
    <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_320px]">
      {/* 主体：刚收到的短信 */}
      <InboxList messages={items} loading={loading && items.length === 0} />

      {/* 辅助：信号 + 事件 */}
      <aside className="flex flex-col gap-4">
        <SignalCard />
        <EventCard events={events} loading={loading && events.length === 0} />
      </aside>
    </div>
  );
}

/* ============================== 短信主体 ============================== */

function InboxList({
  messages,
  loading,
}: {
  messages: SmsMessage[];
  loading: boolean;
}) {
  return (
    <section className="min-w-0">
      {/* 列表直接顶到首页，不再加「收件箱 / 最近接收的短信」小标题——
          卡片本身就是收件箱，重复解释徒增视觉噪声。 */}
      {loading ? (
        <div className="flex flex-col gap-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <Card key={i} className="gap-0 py-0">
              <CardContent className="flex items-start gap-3 p-4">
                <Skeleton className="size-10 rounded-full" />
                <div className="flex-1 space-y-2">
                  <Skeleton className="h-4 w-1/3" />
                  <Skeleton className="h-3 w-4/5" />
                  <Skeleton className="h-3 w-2/3" />
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : messages.length === 0 ? (
        <Card>
          <CardContent className="text-muted-foreground py-16 text-center text-sm">
            尚未接收到短信。设备就绪后新短信会自动出现在这里。
          </CardContent>
        </Card>
      ) : (
        <ul className="flex flex-col gap-3">
          {messages.slice(0, 10).map((m) => (
            <li key={m.id}>
              <InboxCard m={m} />
            </li>
          ))}
        </ul>
      )}
      {/* 「全部记录」底部入口：不抢首屏，需要深挖时再点 */}
      <div className="mt-3 flex justify-center px-1">
        <Button asChild variant="ghost" size="sm" className="text-xs">
          <Link to="/messages">
            全部记录
            <ArrowRight className="size-3" />
          </Link>
        </Button>
      </div>
    </section>
  );
}

/**
 * 单条短信独立卡片。首页主体单位。
 *
 * 视觉层次：
 *   1. **头像色彩差异化**：发件人 hash → 稳定配色，形成视觉记忆
 *   2. **状态色点**：header 行的状态色圆点，按 status 着色一眼区分
 *   3. **hover 动效**：卡片浮起 + 边框变 primary/30 + 阴影加深
 *   4. **正文高亮强化**：验证码块黄底黑字荧光笔感（详见 SmsContent）
 */
function InboxCard({ m }: { m: SmsMessage }) {
  const color = avatarColor(m.sender);
  const accent = statusAccent(m.status);

  return (
    <Card
      className="gap-0 py-0 transition-all duration-200 hover:-translate-y-0.5 hover:border-primary/30 hover:shadow-md"
    >
      <CardContent className="flex flex-col gap-2 p-4">
        {/* header */}
        <div className="flex items-center gap-3">
          <Avatar className="size-9 shrink-0">
            <AvatarFallback
              className="text-xs font-semibold"
              style={{ backgroundColor: color.bg, color: color.fg }}
            >
              {initials(m.sender)}
            </AvatarFallback>
          </Avatar>
          <span className="min-w-0 flex-1 truncate text-sm font-semibold">
            {m.sender ?? "未知发件人"}
          </span>
          <span
            title={m.status}
            className="size-2 shrink-0 rounded-full"
            style={{ backgroundColor: accent }}
          />
          <span className="text-muted-foreground shrink-0 text-xs tabular-nums">
            {fmtAgo(m.received_at)}
          </span>
        </div>
        {/* body：正文 */}
        <SmsContent text={m.content ?? ""} className="text-foreground/85" />
      </CardContent>
    </Card>
  );
}

/* ============================== 信号卡 ============================== */

function SignalCard() {
  const status = useModem((s) => s.status);
  const loading = useModem((s) => s.loading);

  return (
    <Card className="gap-0 py-0">
      <div className="flex items-center gap-2 border-b p-4 pb-3">
        <SignalHigh className="text-primary size-4" />
        <h3 className="text-sm font-semibold">信号</h3>
        <span className="text-muted-foreground ml-auto text-xs tabular-nums">
          {status ? `CSQ ${status.csq ?? "—"}` : "—"}
        </span>
      </div>
      <CardContent className="space-y-3 p-4">
        {loading && !status ? (
          <>
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-4 w-1/3" />
          </>
        ) : (
          <>
            <SignalBar value={status?.csq ?? null} />
            <div className="grid grid-cols-2 gap-2 text-xs">
              <ModemFlag
                icon={Cpu}
                label="SIM"
                ok={status?.sim_ready ?? false}
                okText="就绪"
                badText="异常"
              />
              <ModemFlag
                icon={Wifi}
                label="网络"
                ok={status?.registered ?? false}
                okText="已注册"
                badText="未注册"
              />
            </div>
            <div className="text-muted-foreground flex items-center justify-between text-xs">
              <span className="truncate">{status?.operator ?? "—"}</span>
              <span className="shrink-0 tabular-nums">
                {status?.rssi_dbm ? `${status.rssi_dbm} dBm` : "—"}
              </span>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

function SignalBar({ value }: { value: number | null }) {
  const ratio = csqRatio(value);
  return (
    <div>
      <div className="flex items-end gap-1">
        {Array.from({ length: 12 }).map((_, i) => {
          const filled = i < Math.round(ratio * 12);
          const tone =
            i < 4 ? "bg-destructive" : i < 8 ? "bg-[var(--warning)]" : "bg-[var(--success)]";
          return (
            <div
              key={i}
              className={cn(
                "flex-1 rounded-sm transition-colors",
                filled ? tone : "bg-muted",
              )}
              style={{ height: `${6 + i * 2}px` }}
            />
          );
        })}
      </div>
      <div className="text-muted-foreground mt-1 flex justify-between text-[10px] tabular-nums">
        <span>{csqLabel(value)}</span>
      </div>
    </div>
  );
}

function ModemFlag({
  icon: Icon,
  label,
  ok,
  okText,
  badText,
}: {
  icon: LucideIcon;
  label: string;
  ok: boolean;
  okText: string;
  badText: string;
}) {
  return (
    <div className="bg-muted/40 flex items-center gap-1.5 rounded-md px-2 py-1.5">
      <Icon className={cn("size-3.5", ok ? "text-[var(--success)]" : "text-destructive")} />
      <span className="text-muted-foreground">{label}</span>
      <span className={cn("ml-auto font-medium", ok ? "text-foreground" : "text-destructive")}>
        {ok ? okText : badText}
      </span>
    </div>
  );
}

/* ============================== 事件卡 ============================== */

const EVENT_ICON: Record<string, LucideIcon> = {
  started: Power,
  sim_ready: Cpu,
  registered: Wifi,
  signal: SignalHigh,
  new_message: Bell,
};

function EventCard({
  events,
  loading,
}: {
  events: ModemEventRecord[];
  loading: boolean;
}) {
  return (
    <Card className="gap-0 py-0">
      <div className="flex items-center gap-2 border-b p-4 pb-3">
        <Activity className="text-muted-foreground size-4" />
        <h3 className="text-sm font-semibold">事件</h3>
        <Button asChild variant="link" size="sm" className="ml-auto h-auto p-0 text-xs">
          <Link to="/system">
            全部
            <ArrowRight className="size-3" />
          </Link>
        </Button>
      </div>
      <CardContent className="p-0">
        {loading ? (
          <ul className="divide-y">
            {Array.from({ length: 4 }).map((_, i) => (
              <li key={i} className="flex items-center gap-2 px-4 py-2.5">
                <Skeleton className="size-6 rounded-full" />
                <div className="flex-1 space-y-1.5">
                  <Skeleton className="h-2.5 w-1/2" />
                </div>
              </li>
            ))}
          </ul>
        ) : events.length === 0 ? (
          <div className="text-muted-foreground px-4 py-8 text-center text-xs">
            暂无事件
          </div>
        ) : (
          <ul className="divide-y">
            {events.slice(0, 6).map((e) => {
              const Icon = EVENT_ICON[e.event_type] ?? Activity;
              return (
                <li key={e.id} className="flex items-center gap-2 px-4 py-2.5">
                  <div className="bg-muted text-muted-foreground grid size-6 shrink-0 place-items-center rounded-full">
                    <Icon className="size-3" />
                  </div>
                  <span className="font-mono text-[11px] font-medium">
                    {e.event_type}
                  </span>
                  <span className="text-muted-foreground ml-auto shrink-0 text-[10px] tabular-nums">
                    {fmtTime(e.created_at).slice(11, 16)}
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
