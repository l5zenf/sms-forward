import { useEffect, useRef, useState } from "react";
import {
  Search,
  ChevronRight,
  Loader2,
  Clock,
  Hash,
  AlertCircle,
  Terminal,
} from "lucide-react";

import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import {
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Skeleton } from "@/components/ui/skeleton";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/status-badge";
import { SmsContent } from "@/components/sms-content";
import { useMessages, type StatusFilter } from "@/store/messages";
import { fmtAgo, fmtTime, initials, preview } from "@/utils";
import type { SmsMessage } from "@/types";

/**
 * 信息页只用 3 个分类（已完成 / 处理中 / 已失败），不再像以前那样把每个
 * 底层 status 都做成一个 tab 来选。`all` 留给首页只读流。
 *
 * 默认选「已完成」——大多数时候用户来这里就是查「发过的短信」。
 */
const FILTERS: { key: Exclude<StatusFilter, "all">; label: string }[] = [
  { key: "completed", label: "已完成" },
  { key: "processing", label: "处理中" },
  { key: "failed", label: "已失败" },
];

export default function Messages() {
  const {
    items,
    total,
    loading,
    loadingMore,
    statusFilter,
    query,
    selectedId,
    setStatusFilter,
    setQuery,
    loadMore,
    openDetail,
  } = useMessages();

  const sentinelRef = useRef<HTMLDivElement | null>(null);

  // 首次进入信息页：把全局 statusFilter 从默认的 "all"（首页用）切到
  // "completed"。其他状态切换之后保持用户选择（不强制重置）。
  const initRef = useRef(false);
  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;
    if (useMessages.getState().statusFilter === "all") {
      setStatusFilter("completed");
    }
  }, [setStatusFilter]);

  // 触底自动加载更多。root 是最近的可滚动祖先（main 区）。
  const loadMoreRef = useRef(loadMore);
  loadMoreRef.current = loadMore;
  useEffect(() => {
    const el = sentinelRef.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) void loadMoreRef.current();
      },
      // 触发线：哨兵进入视口前 300px 就开始预取，减少等待感
      { rootMargin: "300px 0px 0px 0px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [items.length > 0]); // 仅在首屏有数据后挂一次；deps 用 bool 避免重渲染抖动

  const hasMore = items.length < total;

  return (
    <div className="flex flex-col gap-4">
      <Card className="gap-0">
        {/* 工具条：搜索 + 状态过滤 */}
        <CardContent className="sticky top-0 z-10 flex flex-col gap-3 bg-card/95 py-3 backdrop-blur supports-[backdrop-filter]:bg-card/75">
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="relative w-full md:max-w-xs">
              <Search className="text-muted-foreground absolute left-2.5 top-1/2 size-4 -translate-y-1/2" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="搜索发件人或内容…"
                className="pl-8"
              />
            </div>
            <Tabs
              value={statusFilter}
              onValueChange={(v) => setStatusFilter(v as StatusFilter)}
            >
              <TabsList className="flex-wrap">
                {FILTERS.map((f) => (
                  <TabsTrigger key={f.key} value={f.key}>
                    {f.label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
          </div>
        </CardContent>

        <Separator />

        {/* 列表：行可就地展开（手风琴），无限滚动 */}
        <CardContent className="p-0">
          {loading && items.length === 0 ? (
            <ul className="divide-y">
              {Array.from({ length: 5 }).map((_, i) => (
                <li key={i} className="flex items-center gap-3 px-4 py-3">
                  <Skeleton className="size-10 rounded-full" />
                  <div className="flex-1 space-y-2">
                    <Skeleton className="h-3 w-1/4" />
                    <Skeleton className="h-3 w-2/3" />
                  </div>
                </li>
              ))}
            </ul>
          ) : items.length === 0 ? (
            <div className="text-muted-foreground px-4 py-16 text-center text-sm">
              没有匹配的短信
            </div>
          ) : (
            <ul className="divide-y">
              {items.map((m) => (
                <li key={m.id}>
                  <MessageRow
                    m={m}
                    expanded={selectedId === m.id}
                    onToggle={() =>
                      openDetail(selectedId === m.id ? null : m.id)
                    }
                  />
                </li>
              ))}
            </ul>
          )}

          {/* 触底哨兵：永远在列表内，IntersectionObserver 检测它进入视口 */}
          {hasMore && !loading && (
            <div ref={sentinelRef} className="flex justify-center py-4">
              {loadingMore && (
                <span className="text-muted-foreground flex items-center gap-2 text-xs">
                  <Loader2 className="size-3.5 animate-spin" />
                  加载更多…
                </span>
              )}
            </div>
          )}
          {!hasMore && items.length > 0 && (
            <div className="text-muted-foreground px-4 py-4 text-center text-xs">
              已加载全部 {total} 条
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

/**
 * 单条短信行（折叠态）。
 *
 * 减负原则：一行只承载「发件人是谁 + 当前何态 + 何时收到」三件事。
 *   - 头像：视觉锚点
 *   - 发件人：主标题
 *   - 状态徽标：彩色 status pill
 *   - 相对时间：右对齐，一眼是否最新
 *   - 预览文本：很轻的灰字，仅供识别用
 * 「重试 N」「#id」等多字段一律移到详情，避免行密度噪音。
 * chevron 旋转 + 行底色作为展开反馈。
 */
function MessageRow({
  m,
  expanded,
  onToggle,
}: {
  m: SmsMessage;
  expanded: boolean;
  onToggle: () => void;
}) {
  return (
    <div>
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={expanded}
        className={cn(
          "hover:bg-accent flex w-full items-center gap-3 px-4 py-3 text-left transition-colors",
          expanded && "bg-accent/60",
        )}
      >
        <Avatar className="size-10 shrink-0">
          <AvatarFallback className="bg-secondary text-secondary-foreground text-xs font-medium">
            {initials(m.sender)}
          </AvatarFallback>
        </Avatar>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-medium">
              {m.sender ?? "未知发件人"}
            </span>
            {/* 不再展示 StatusBadge——tab 已隐含状态分类，行内冗余 */}
            <span className="text-muted-foreground ml-auto shrink-0 text-xs tabular-nums">
              {fmtAgo(m.received_at)}
            </span>
          </div>
          {!expanded && (
            <p className="text-muted-foreground mt-0.5 truncate text-xs">
              {preview(m.content)}
            </p>
          )}
        </div>
        <ChevronRight
          className={cn(
            "text-muted-foreground size-4 shrink-0 transition-transform",
            expanded && "rotate-90",
          )}
        />
      </button>

      {expanded && <MessageDetail m={m} />}
    </div>
  );
}

/**
 * 展开后的详情区（就地）。
 *
 * 信息分层（从上到下，按视野权重）：
 *   1. 全文主体（验证码加粗可复制）——最重要
 *   2. 摘要条：状态 + 绝对时间 + 「第 N 条」——常查字段
 *   3. 故障区：last_error / decode 等异常独占，明显但克制
 *   4. 技术细节：ICCID、去重键、PDU——折叠，开发者才看
 *
 * 用 useState 局部控制「技术细节」折叠（不依赖 store，避免污染全局）。
 */
function MessageDetail({ m }: { m: SmsMessage }) {
  const [showTech, setShowTech] = useState(false);
  const isFailed = m.status === "failed" || m.status === "decode_failed";
  const failedHint =
    m.status === "decode_failed"
      ? "短信内容解析失败（PDU 编码异常）"
      : m.last_error ?? null;

  return (
    <div className="bg-muted/20 space-y-3 px-4 pb-4 pt-2">
      {/* 1. 正文主区 */}
      <SmsContent text={m.content ?? "(内容为空)"} className="text-foreground/90 text-sm" />

      {/* 2. 摘要条：状态 pill + 时间 + id */}
      <div className="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs">
        <StatusBadge status={m.status} />
        <span className="inline-flex items-center gap-1 tabular-nums">
          <Clock className="size-3" />
          {fmtTime(m.received_at)}
        </span>
        {m.sms_time && (
          <span className="tabular-nums">SMS {fmtTime(m.sms_time)}</span>
        )}
        {m.forwarded_at && (
          <span className="tabular-nums">转发于 {fmtTime(m.forwarded_at)}</span>
        )}
        <span className="inline-flex items-center gap-1">
          <Hash className="size-3" />
          {m.id}
        </span>
        {m.retry_count > 0 && (
          <span className="tabular-nums">
            重试 {m.retry_count}
            {m.max_retry ? ` / ${m.max_retry}` : ""}
          </span>
        )}
      </div>

      {/* 3. 故障区：失败/解析失败独占一区，红色块醒目 */}
      {isFailed && failedHint && (
        <div className="bg-destructive/10 border-destructive/30 text-destructive flex items-start gap-2 rounded-md border px-3 py-2 text-xs">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          <span className="font-medium">{failedHint}</span>
        </div>
      )}

      {/* 4. 技术细节：ICCID/去重键/PDU，默认折叠 */}
      {(m.iccid || m.dedupe_key || m.pdu_raw) && (
        <div className="border-t pt-2">
          <button
            type="button"
            onClick={() => setShowTech((v) => !v)}
            className="text-muted-foreground hover:text-foreground flex w-full items-center gap-1.5 text-xs font-medium transition-colors"
          >
            <Terminal className="size-3.5" />
            技术细节
            <ChevronRight
              className={cn("ml-auto size-3.5 transition-transform", showTech && "rotate-90")}
            />
          </button>
          {showTech && (
            <dl className="mt-2 space-y-1 text-[11px]">
              {m.iccid && (
                <div className="flex gap-2">
                  <dt className="text-muted-foreground w-16 shrink-0">ICCID</dt>
                  <dd className="font-mono break-all">{m.iccid}</dd>
                </div>
              )}
              {m.dedupe_key && (
                <div className="flex gap-2">
                  <dt className="text-muted-foreground w-16 shrink-0">去重键</dt>
                  <dd className="font-mono break-all">{m.dedupe_key}</dd>
                </div>
              )}
              {m.pdu_raw && (
                <div className="flex gap-2">
                  <dt className="text-muted-foreground w-16 shrink-0">PDU</dt>
                  <dd className="min-w-0 flex-1">
                    <pre className="bg-muted/40 max-h-48 overflow-auto rounded p-2 font-mono text-[10px] leading-relaxed break-all">
                      {m.pdu_raw}
                    </pre>
                  </dd>
                </div>
              )}
            </dl>
          )}
        </div>
      )}
    </div>
  );
}
