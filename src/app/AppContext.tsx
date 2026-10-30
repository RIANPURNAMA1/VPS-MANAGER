import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, Info, ShieldAlert, XCircle } from "lucide-react";
import { api, toAppErr } from "../lib/api";
import type { AppErr } from "../lib/api";
import { applyTheme, loadTheme, saveTheme } from "../lib/theme";
import type { ThemeChoice } from "../lib/theme";
import type { AppSettings, Profile } from "../types";
import { Button, Modal } from "../components/ui";

export type Page =
  | "overview" | "servers" | "websites" | "services" | "terminal" | "files"
  | "databases" | "deployments" | "assistant" | "logs" | "settings";

interface Toast { id: number; kind: "success" | "error" | "info"; text: string }

interface Ctx {
  page: Page;
  go: (p: Page) => void;
  profiles: Profile[];
  reloadProfiles: () => Promise<void>;
  activeId: number | null;
  setActiveId: (id: number | null) => void;
  active: Profile | null;
  connected: Set<number>;
  isConnected: boolean;
  connect: (id: number) => Promise<boolean>;
  disconnect: (id: number) => Promise<void>;
  withTrust: <T>(fn: (trust?: string) => Promise<T>) => Promise<T>;
  settings: AppSettings | null;
  setSettings: (s: AppSettings) => void;
  theme: ThemeChoice;
  setTheme: (t: ThemeChoice) => void;
  notify: (kind: Toast["kind"], text: string) => void;
  fail: (e: unknown, prefix?: string) => void;
  refreshTick: number;
  refresh: () => void;
}

const AppCtx = createContext<Ctx | null>(null);
export function useApp(): Ctx {
  const c = useContext(AppCtx);
  if (!c) throw new Error("useApp harus dipakai di dalam AppProvider");
  return c;
}

type HostPrompt = { data: NonNullable<AppErr["data"]>; resolve: (fp: string | null) => void } | null;

const ACTIVE_KEY = "vpsm.active";

export function AppProvider({ children }: { children: ReactNode }) {
  const [page, setPage] = useState<Page>("overview");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [activeId, setActiveIdState] = useState<number | null>(() => {
    try { const v = localStorage.getItem(ACTIVE_KEY); return v ? Number(v) : null; } catch { return null; }
  });
  const [connected, setConnected] = useState<Set<number>>(new Set());
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [theme, setThemeState] = useState<ThemeChoice>(loadTheme);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [hostPrompt, setHostPrompt] = useState<HostPrompt>(null);
  const [changedHost, setChangedHost] = useState<AppErr | null>(null);
  const [refreshTick, setTick] = useState(0);
  const tid = useRef(1);

  const notify = useCallback((kind: Toast["kind"], text: string) => {
    const id = tid.current++;
    setToasts((t) => [...t.slice(-3), { id, kind, text }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 8000 : 4500);
  }, []);
  const fail = useCallback((e: unknown, prefix?: string) => {
    const err = toAppErr(e);
    notify("error", prefix ? `${prefix}: ${err.message}` : err.message);
  }, [notify]);

  const reloadProfiles = useCallback(async () => {
    try {
      setProfiles(await api.listProfiles());
      setConnected(new Set(await api.connectionStates()));
    } catch (e) { fail(e, "Gagal memuat server"); }
  }, [fail]);

  // inisialisasi
  useEffect(() => {
    void reloadProfiles();
    api.settingsGet().then((s) => {
      setSettings(s);
    }).catch(() => undefined);
  }, [reloadProfiles]);

  // tema
  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const h = () => applyTheme("system");
    mq.addEventListener("change", h);
    return () => mq.removeEventListener("change", h);
  }, [theme]);
  const setTheme = useCallback((t: ThemeChoice) => { saveTheme(t); setThemeState(t); }, []);

  const setActiveId = useCallback((id: number | null) => {
    setActiveIdState(id);
    try { if (id == null) localStorage.removeItem(ACTIVE_KEY); else localStorage.setItem(ACTIVE_KEY, String(id)); } catch { /* abaikan */ }
  }, []);
  useEffect(() => {
    if (profiles.length && (activeId == null || !profiles.some((p) => p.id === activeId))) setActiveId(profiles[0]!.id);
    if (!profiles.length && activeId != null) setActiveId(null);
  }, [profiles, activeId, setActiveId]);

  const withTrust = useCallback(async <T,>(fn: (trust?: string) => Promise<T>): Promise<T> => {
    try {
      return await fn();
    } catch (e) {
      const err = toAppErr(e);
      if (err.code === "host_key_unknown" && err.data) {
        const fp = await new Promise<string | null>((resolve) => setHostPrompt({ data: err.data!, resolve }));
        setHostPrompt(null);
        if (!fp) throw { code: "cancelled", message: "Koneksi dibatalkan: host key tidak disetujui." } satisfies AppErr;
        return await fn(fp);
      }
      if (err.code === "host_key_changed") setChangedHost(err);
      throw err;
    }
  }, []);

  const connect = useCallback(async (id: number) => {
    try {
      const ms = await withTrust((fp) => api.connect(id, fp));
      setConnected((s) => new Set(s).add(id));
      notify("success", `Terhubung (${ms} ms).`);
      return true;
    } catch (e) {
      const err = toAppErr(e);
      if (err.code !== "cancelled") fail(err, "Gagal terhubung");
      return false;
    }
  }, [withTrust, notify, fail]);

  const disconnect = useCallback(async (id: number) => {
    try {
      await api.disconnect(id);
      setConnected((s) => { const n = new Set(s); n.delete(id); return n; });
      notify("info", "Koneksi diputus.");
    } catch (e) { fail(e); }
  }, [notify, fail]);

  // pantau koneksi aktif
  const activeConnected = activeId != null && connected.has(activeId);
  useEffect(() => {
    if (!activeConnected || activeId == null) return;
    const t = window.setInterval(async () => {
      try { await api.checkConnection(activeId); } catch (e) {
        setConnected((s) => { const n = new Set(s); n.delete(activeId); return n; });
        notify("error", toAppErr(e).message);
      }
    }, 25000);
    return () => window.clearInterval(t);
  }, [activeConnected, activeId, notify]);

  const active = profiles.find((p) => p.id === activeId) ?? null;
  const value = useMemo<Ctx>(() => ({
    page, go: setPage, profiles, reloadProfiles, activeId, setActiveId, active, connected,
    isConnected: activeConnected, connect, disconnect, withTrust, settings, setSettings, theme, setTheme,
    notify, fail, refreshTick, refresh: () => setTick((n) => n + 1),
  }), [page, profiles, reloadProfiles, activeId, setActiveId, active, connected, activeConnected, connect, disconnect, withTrust, settings, theme, setTheme, notify, fail, refreshTick]);

  const icon = { success: <CheckCircle2 className="size-4" />, error: <XCircle className="size-4" />, info: <Info className="size-4" /> };
  return (
    <AppCtx.Provider value={value}>
      {children}
      <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-[360px] flex-col gap-2" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} role={t.kind === "error" ? "alert" : "status"}
            className={`pointer-events-auto flex items-start gap-2 rounded-md border bg-card p-3 text-[13px] ${t.kind === "error" ? "border-dashed border-fg" : "border-line"}`}>
            <span className="mt-0.5 shrink-0" aria-hidden>{icon[t.kind]}</span>
            <span className="break-words">{t.text}</span>
          </div>
        ))}
      </div>
      {hostPrompt && (
        <Modal title="Verifikasi host key server" onClose={() => hostPrompt.resolve(null)}
          footer={<>
            <Button onClick={() => hostPrompt.resolve(null)}>Batal</Button>
            <Button variant="primary" onClick={() => hostPrompt.resolve(String(hostPrompt.data.fingerprint))}>Percayai dan lanjutkan</Button>
          </>}>
          <div className="space-y-3">
            <p>Ini pertama kalinya Anda terhubung ke <b>{String(hostPrompt.data.host)}:{String(hostPrompt.data.port)}</b>. Bandingkan fingerprint di bawah dengan yang ada di server (<code className="font-mono">ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code>) sebelum melanjutkan.</p>
            <dl className="rounded-md border border-line bg-sidebar p-3 font-mono text-[13px]">
              <dt className="text-fg2">Tipe</dt><dd className="mb-2">{String(hostPrompt.data.key_type)}</dd>
              <dt className="text-fg2">Fingerprint</dt><dd className="break-all">{String(hostPrompt.data.fingerprint)}</dd>
            </dl>
          </div>
        </Modal>
      )}
      {changedHost && (
        <Modal title="Host key berubah — koneksi diblokir" onClose={() => setChangedHost(null)}
          footer={<Button variant="primary" onClick={() => setChangedHost(null)}>Mengerti</Button>}>
          <div className="space-y-3">
            <p className="flex items-start gap-2 font-medium"><ShieldAlert className="mt-0.5 size-4 shrink-0" /> Server mengirim host key yang berbeda dari yang pernah Anda setujui.</p>
            <p className="text-fg2">Ini bisa terjadi jika server di-install ulang, tetapi juga bisa menandakan serangan man-in-the-middle. Jika Anda yakin server memang diganti, hapus host key lama di <b>Pengaturan → Host terpercaya</b>, lalu hubungkan ulang.</p>
            <dl className="rounded-md border border-line bg-sidebar p-3 font-mono text-[13px]">
              <dt className="text-fg2">Fingerprint tersimpan</dt><dd className="mb-2 break-all">{String(changedHost.data?.known_fingerprint)}</dd>
              <dt className="text-fg2">Fingerprint dari server</dt><dd className="break-all">{String(changedHost.data?.fingerprint)}</dd>
            </dl>
            <p className="flex items-center gap-2 text-fg2"><AlertTriangle className="size-4" /> Koneksi tidak dilanjutkan.</p>
          </div>
        </Modal>
      )}
    </AppCtx.Provider>
  );
}
