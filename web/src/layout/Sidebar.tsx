import { useState } from "react";
import { NavLink, useLocation } from "react-router-dom";
import {
  LayoutDashboard,
  MessageSquare,
  Settings,
  ChevronLeft,
  Radio,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useMessages } from "@/store/messages";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[] = [
  { to: "/", label: "首页", icon: LayoutDashboard },
  { to: "/messages", label: "信息", icon: MessageSquare },
  { to: "/settings", label: "设置", icon: Settings },
];

const STORAGE_KEY = "gg-guard.sidebar.collapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function Sidebar() {
  const [collapsed, setCollapsed] = useState<boolean>(readCollapsed);
  const stats = useMessages((s) => s.stats);
  // 角标只反映「待转发」存量。失败的转发对用户不是重点（见首页 InboxRow
  // 的色点设计），这里不算入角标，避免副作用是「红色警报」的观感。
  const pending = stats?.pending ?? 0;

  const toggle = () => {
    const next = !collapsed;
    setCollapsed(next);
    try {
      localStorage.setItem(STORAGE_KEY, next ? "1" : "0");
    } catch {
      /* ignore */
    }
  };

  return (
    <aside
      className={cn(
        "bg-sidebar text-sidebar-foreground flex flex-col border-r transition-[width] duration-200 ease-in-out",
        collapsed ? "w-16" : "w-60"
      )}
    >
      {/* Brand */}
      <div
        className={cn(
          "flex h-16 items-center gap-3 px-4",
          collapsed && "justify-center px-0"
        )}
      >
        <div className="bg-primary text-primary-foreground grid size-9 shrink-0 place-items-center rounded-lg shadow-sm">
          <Radio className="size-5" />
        </div>
        {!collapsed && (
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold">gg-guard</div>
            <div className="text-muted-foreground truncate text-xs">
              SMS 守护
            </div>
          </div>
        )}
      </div>

      <Separator />

      {/* Nav */}
      <nav className="flex flex-1 flex-col gap-1 p-2">
        <TooltipProvider delayDuration={150}>
          {NAV.map((item) => (
            <SidebarLink
              key={item.to}
              item={item}
              collapsed={collapsed}
              pendingBadge={item.to === "/messages" ? pending : 0}
            />
          ))}
        </TooltipProvider>
      </nav>

      {/* Collapse toggle */}
      <div className="p-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={toggle}
          className={cn("w-full justify-center", !collapsed && "justify-end")}
          aria-label={collapsed ? "展开侧边栏" : "折叠侧边栏"}
        >
          <ChevronLeft
            className={cn("size-4 transition-transform", collapsed && "rotate-180")}
          />
          {!collapsed && <span className="text-xs">折叠</span>}
        </Button>
      </div>
    </aside>
  );
}

function SidebarLink({
  item,
  collapsed,
  pendingBadge,
}: {
  item: NavItem;
  collapsed: boolean;
  pendingBadge: number;
}) {
  const Icon = item.icon;
  const link = (
    <NavLink to={item.to} end className="block">
      {({ isActive }) => (
        <span
          className={cn(
            "group hover:bg-sidebar-accent hover:text-sidebar-accent-foreground flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
            collapsed && "justify-center px-0",
            isActive
              ? "bg-sidebar-accent text-sidebar-accent-foreground"
              : "text-sidebar-foreground/70"
          )}
        >
          <Icon
            className={cn(
              "size-4 shrink-0",
              isActive && "text-primary"
            )}
          />
          {!collapsed && <span className="truncate">{item.label}</span>}
          {!collapsed && pendingBadge > 0 && (
            <Badge variant="secondary" className="ml-auto h-5 px-1.5 text-[10px] tabular-nums">
              {pendingBadge}
            </Badge>
          )}
          {collapsed && pendingBadge > 0 && (
            <span className="bg-primary absolute top-1 right-1 size-2 rounded-full" />
          )}
        </span>
      )}
    </NavLink>
  );

  // Need relative positioning for the collapsed dot
  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="relative">{link}</div>
        </TooltipTrigger>
        <TooltipContent side="right">{item.label}</TooltipContent>
      </Tooltip>
    );
  }
  return link;
}

export function usePageMeta(): { title: string; sub: string } {
  const loc = useLocation();
  if (loc.pathname.startsWith("/messages"))
    return { title: "信息", sub: "已接收的短信记录" };
  if (loc.pathname.startsWith("/settings"))
    return { title: "设置", sub: "设备与应用配置" };
  return { title: "首页", sub: "" };
}
