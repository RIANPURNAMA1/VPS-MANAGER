import { useEffect, useRef, useState } from "react";
import {
  Activity, Bot, Database, FileText, Globe, LayoutDashboard, Moon, RefreshCw, Rocket, Server, Settings as SettingsIcon,
  Sun, SquareTerminal, Wrench, Monitor, ChevronRight, UserRound,
} from "lucide-react";
import type { ReactNode } from "react";
import { useApp } from "./AppContext";
import type { Page } from "./AppContext";
import { Badge, IconButton, cx } from "../components/ui";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { ConnectionsPage } from "../features/connections/ConnectionsPage";
import { WebsitesPage } from "../features/websites/WebsitesPage";
import { ServicesPage } from "../features/services/ServicesPage";
import { TerminalPage } from "../features/terminal/TerminalPage";
import { FilesPage } from "../features/files/FilesPage";
import { DatabasesPage } from "../features/databases/DatabasesPage";
import { DeploymentsPage } from "../features/deployments/DeploymentsPage";
import { AssistantPage } from "../features/ai-assistant/AssistantPage";
import { LogsPage } from "../features/logs/LogsPage";
import { SettingsPage } from "../features/settings/SettingsPage";

const NAV: Array<{ id: Page; label: string; icon: ReactNode }> = [
  { id: "overview", label: "Overview", icon: <LayoutDashboard className="size-4" /> },
  { id: "servers", label: "Servers", icon: <Server className="size-4" /> },
  { id: "websites", label: "Websites", icon: <Globe className="size-4" /> },
  { id: "services", label: "Services", icon: <Wrench className="size-4" /> },
  { id: "terminal", label: "Terminal", icon: <SquareTerminal className="size-4" /> },
  { id: "files", label: "File Manager", icon: <FileText className="size-4" /> },
  { id: "databases", label: "Databases", icon: <Database className="size-4" /> },
  { id: "deployments", label: "Deployments", icon: <Rocket className="size-4" /> },
  { id: "assistant", label: "AI Assistant", icon: <Bot className="size-4" /> },
  { id: "logs", label: "Activity Logs", icon: <Activity className="size-4" /> },
  { id: "settings", label: "Settings", icon: <SettingsIcon className="size-4" /> },
];

const TITLES: Record<Page, string> = Object.fromEntries(NAV.map((n) => [n.id, n.label])) as Record<Page, string>;

function Sidebar() {
  const { page, go } = useApp();
  return (
    <nav aria-label="Navigasi utama" className="flex w-56 shrink-0 flex-col border-r border-line bg-sidebar">
      <div className="flex h-14 items-center gap-2.5 border-b border-line px-4">
        <div className="flex size-7 items-center justify-center rounded-md bg-fg text-bg"><Server className="size-4" aria-hidden /></div>
        <span className="text-[15px] font-semibold">VPS Manager</span>
      </div>
      <ul className="flex-1 space-y-0.5 overflow-y-auto p-2">
        {NAV.map((n) => (
          <li key={n.id}>
            <button type="button" onClick={() => go(n.id)} aria-current={page === n.id ? "page" : undefined}
              className={cx("flex h-9 w-full items-center gap-2.5 rounded-md px-3 text-left", page === n.id ? "bg-hover font-semibold text-fg" : "text-fg2 hover:bg-hover hover:text-fg")}>
              {n.icon}{n.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}

function ThemeToggle() {
  const { theme, setTheme } = useApp();
  const next = theme === "light" ? "dark" : theme === "dark" ? "system" : "light";
  const icon = theme === "light" ? <Sun className="size-4" /> : theme === "dark" ? <Moon className="size-4" /> : <Monitor className="size-4" />;
  const name = { light: "Terang", dark: "Gelap", system: "Sistem" }[theme];
  return <IconButton label={`Tema: ${name} (klik untuk mengganti)`} onClick={() => setTheme(next)}>{icon}</IconButton>;
}

function Topbar() {
  const { page, go, profiles, active, setActiveId, isConnected, connect, disconnect, refresh } = useApp();
  const [busy, setBusy] = useState(false);
  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-4 border-b border-line px-6">
      <div className="min-w-0">
        <nav aria-label="Breadcrumb" className="flex items-center gap-1 text-xs text-fg2">
          <span>VPS Manager</span><ChevronRight className="size-3" aria-hidden /><span>{active?.name ?? "Tanpa server"}</span>
        </nav>
        <h1 className="truncate text-[15px] font-semibold leading-tight">{TITLES[page]}</h1>
      </div>
      <div className="flex items-center gap-3">
        {profiles.length > 0 && (
          <select aria-label="Server aktif" value={active?.id ?? ""} onChange={(e) => setActiveId(Number(e.target.value))}
            className="h-8 max-w-48 rounded-md border border-line bg-bg px-2 text-[13px]">
            {profiles.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        )}
        {active && (
          <>
            <Badge tone={isConnected ? "ok" : "neutral"}>{isConnected ? "Terhubung" : "Tidak terhubung"}</Badge>
            <button type="button" disabled={busy} className="rounded-md border border-line px-2.5 py-1 text-[13px] hover:bg-hover disabled:opacity-50"
              onClick={async () => { setBusy(true); try { if (isConnected) await disconnect(active.id); else await connect(active.id); } finally { setBusy(false); } }}>
              {isConnected ? "Putuskan" : "Hubungkan"}
            </button>
          </>
        )}
        <IconButton label="Muat ulang halaman" onClick={refresh}><RefreshCw className="size-4" /></IconButton>
        <ThemeToggle />
        <IconButton label="Pengaturan aplikasi" onClick={() => go("settings")}><UserRound className="size-4" /></IconButton>
      </div>
    </header>
  );
}

function PageView() {
  const { page, refreshTick } = useApp();
  const key = `${page}-${refreshTick}`;
  switch (page) {
    case "overview": return <DashboardPage key={key} />;
    case "servers": return <ConnectionsPage key={key} />;
    case "websites": return <WebsitesPage key={key} />;
    case "services": return <ServicesPage key={key} />;
    case "files": return <FilesPage key={key} />;
    case "databases": return <DatabasesPage key={key} />;
    case "deployments": return <DeploymentsPage key={key} />;
    case "assistant": return <AssistantPage key={key} />;
    case "logs": return <LogsPage key={key} />;
    case "settings": return <SettingsPage key={key} />;
    default: return null;
  }
}

export function App() {
  const { page } = useApp();
  const mainRef = useRef<HTMLElement>(null);
  useEffect(() => { mainRef.current?.focus({ preventScroll: true }); }, [page]);
  return (
    <div className="flex h-full min-w-[960px]">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        <main ref={mainRef} tabIndex={-1} className="min-h-0 flex-1 overflow-y-auto p-6 outline-none">
          {/* Terminal tetap ter-mount agar sesi tidak putus saat pindah halaman */}
          <div hidden={page !== "terminal"} className="h-full"><TerminalPage visible={page === "terminal"} /></div>
          {page !== "terminal" && <PageView />}
        </main>
      </div>
    </div>
  );
}
