import { HashRouter, Route, Routes } from "react-router-dom";
import { useEffect } from "react";
import { AlertCircle } from "lucide-react";

import { Sidebar } from "@/layout/Sidebar";
import { Topbar } from "@/layout/Topbar";
import { Toaster } from "@/components/ui/toaster";
import Dashboard from "@/pages/Dashboard";
import Messages from "@/pages/Messages";
import Modem from "@/pages/Modem";
import System from "@/pages/System";
import Settings from "@/pages/Settings";
import { useMessages } from "@/store/messages";
import { useModem } from "@/store/modem";
import { cn } from "@/lib/utils";

// HashRouter avoids needing history rewrites for unknown static paths when the
// built bundle is served directly by axum.
function Router() {
  return (
    <HashRouter>
      <div className="flex h-screen overflow-hidden">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <Topbar onRefresh={useRefreshAll()} />
          <OfflineBanner />
          <main className="flex-1 overflow-y-auto">
            <div className="mx-auto max-w-7xl p-4 md:p-6">
              <Routes>
                <Route path="/" element={<Dashboard />} />
                <Route path="/messages" element={<Messages />} />
                {/* modem/system 不在侧边栏暴露，但保留路由：首页事件卡「全部」深挖用 */}
                <Route path="/modem" element={<Modem />} />
                <Route path="/system" element={<System />} />
                <Route path="/settings" element={<Settings />} />
                <Route path="*" element={<Dashboard />} />
              </Routes>
            </div>
          </main>
        </div>
      </div>
      <Toaster />
    </HashRouter>
  );
}

function useRefreshAll() {
  const refreshMsgs = useMessages((s) => s.refresh);
  const refreshModem = useModem((s) => s.refresh);
  useEffect(() => {
    void refreshMsgs();
    void refreshModem();
  }, [refreshMsgs, refreshModem]);
  return () => {
    void refreshMsgs();
    void refreshModem();
  };
}

function OfflineBanner() {
  const online = useMessages((s) => s.online);
  const err = useMessages((s) => s.error);
  const hardError = useModem((s) => !s.online && !!s.status);
  return (
    <div
      className={cn(
        "flex items-center gap-2 overflow-hidden border-b px-4 text-sm transition-all md:px-6",
        online
          ? "max-h-0 border-transparent opacity-0"
          : "bg-destructive/10 text-destructive max-h-12 border-destructive/30 py-2 opacity-100"
      )}
      role={online ? undefined : "alert"}
    >
      <AlertCircle className="size-4 shrink-0" />
      <span>无法连接到服务器{err && !hardError ? `：${err}` : ""}</span>
    </div>
  );
}

export default function App() {
  return <Router />;
}
