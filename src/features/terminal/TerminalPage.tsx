import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { ClipboardCopy, Eraser, PlugZap, SquareTerminal } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api, on } from "../../lib/api";
import { isDangerousCommand } from "../../lib/format";
import { Badge, Button, Card, ConfirmDialog, EmptyState, PageHeader } from "../../components/ui";

const MONO_THEME = {
  background: "#000000", foreground: "#ffffff", cursor: "#ffffff", cursorAccent: "#000000", selectionBackground: "#555555",
  black: "#000000", red: "#d0d0d0", green: "#e0e0e0", yellow: "#f0f0f0", blue: "#b8b8b8", magenta: "#c8c8c8", cyan: "#d8d8d8", white: "#ffffff",
  brightBlack: "#777777", brightRed: "#e8e8e8", brightGreen: "#f4f4f4", brightYellow: "#ffffff", brightBlue: "#dcdcdc", brightMagenta: "#e4e4e4", brightCyan: "#eeeeee", brightWhite: "#ffffff",
};

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Tetap ter-mount (hidden) saat pindah halaman, sehingga sesi tidak terputus. */
export function TerminalPage({ visible }: { visible: boolean }) {
  const { active, isConnected, notify, fail, go, connect } = useApp();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const sid = useRef<string | null>(null);
  const line = useRef("");
  const [state, setState] = useState<"idle" | "connecting" | "open" | "closed">("idle");
  const [pending, setPending] = useState<{ text: string; send: () => void; cancel: () => void } | null>(null);
  const serverId = active?.id ?? null;

  const send = useCallback((data: string) => {
    if (sid.current) api.terminalWrite(sid.current, data).catch(() => undefined);
  }, []);

  // buat terminal xterm sekali
  useEffect(() => {
    if (!host.current) return;
    const t = new Terminal({ fontFamily: '"Cascadia Mono", Consolas, monospace', fontSize: 13, cursorBlink: true, scrollback: 10000, theme: MONO_THEME, allowProposedApi: false });
    const f = new FitAddon();
    t.loadAddon(f);
    t.open(host.current);
    term.current = t; fit.current = f;

    t.attachCustomKeyEventHandler((e) => {
      if (e.type === "keydown" && e.ctrlKey && e.shiftKey && e.code === "KeyC") { void navigator.clipboard.writeText(t.getSelection()); return false; }
      if (e.type === "keydown" && e.ctrlKey && !e.shiftKey && e.code === "KeyC" && t.hasSelection()) { void navigator.clipboard.writeText(t.getSelection()); return false; }
      return true;
    });

    const d = t.onData((data) => {
      // Konfirmasi sebelum perintah berpotensi merusak (best-effort: berdasarkan baris yang diketik).
      if (data === "\r") {
        const cmd = line.current; line.current = "";
        if (isDangerousCommand(cmd)) {
          setPending({ text: cmd, send: () => { send("\r"); setPending(null); }, cancel: () => { send("\x15"); setPending(null); t.focus(); } });
          return;
        }
        send(data); return;
      }
      if (data.length > 1 && /[\r\n]/.test(data)) {
        if (isDangerousCommand(data)) {
          setPending({ text: data.trim(), send: () => { send(data); setPending(null); }, cancel: () => { setPending(null); t.focus(); } });
          return;
        }
        line.current = "";
      } else if (data === "\x7f") line.current = line.current.slice(0, -1);
      else if (data === "\x03" || data === "\x15") line.current = "";
      else if (data.length === 1 && data >= " ") line.current += data;
      else if (data.length > 1 && !data.startsWith("\x1b")) line.current += data;
      send(data);
    });
    const r = t.onResize(({ cols, rows }) => { if (sid.current) void api.terminalResize(sid.current, cols, rows).catch(() => undefined); });
    const ro = new ResizeObserver(() => { try { f.fit(); } catch { /* host hidden */ } });
    ro.observe(host.current);

    let un: Array<() => void> = [];
    void Promise.all([
      on<{ id: string; data: string }>("terminal-output", (p) => { if (p.id === sid.current) t.write(b64ToBytes(p.data)); }),
      on<{ id: string }>("terminal-closed", (p) => {
        if (p.id === sid.current) { sid.current = null; setState("closed"); t.writeln("\r\n[Sesi ditutup]"); }
      }),
    ]).then((u) => { un = u; });
    return () => { d.dispose(); r.dispose(); ro.disconnect(); un.forEach((x) => x()); if (sid.current) void api.terminalClose(sid.current); t.dispose(); };
  }, [send]);

  const open = useCallback(async () => {
    if (serverId == null || !term.current || !fit.current) return;
    if (sid.current) { await api.terminalClose(sid.current).catch(() => undefined); sid.current = null; }
    setState("connecting");
    term.current.reset();
    try { fit.current.fit(); } catch { /* abaikan */ }
    try {
      const id = await api.terminalOpen(serverId, term.current.cols, term.current.rows);
      sid.current = id; setState("open"); term.current.focus();
    } catch (e) { setState("closed"); fail(e, "Gagal membuka terminal"); }
  }, [serverId, fail]);

  // buka otomatis saat halaman terlihat dan server terhubung
  useEffect(() => {
    if (visible && isConnected && state === "idle") void open();
    if (visible) requestAnimationFrame(() => { try { fit.current?.fit(); } catch { /* abaikan */ } });
  }, [visible, isConnected, state, open]);

  // server berganti / terputus → tutup sesi
  const prev = useRef<number | null>(serverId);
  useEffect(() => {
    if (prev.current !== serverId || !isConnected) {
      if (sid.current) { void api.terminalClose(sid.current); sid.current = null; }
      if (state !== "idle") setState("idle");
      prev.current = serverId;
    }
  }, [serverId, isConnected, state]);

  const copy = async () => {
    const s = term.current?.getSelection();
    if (!s) { notify("info", "Pilih teks di terminal terlebih dahulu."); return; }
    await navigator.clipboard.writeText(s); notify("success", "Teks disalin.");
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="Terminal" description="Sesi SSH interaktif dengan PTY. Isi terminal tidak pernah dicatat oleh aplikasi."
        actions={<>
          <Badge tone={state === "open" ? "ok" : state === "connecting" ? "active" : "neutral"}>{state === "open" ? "Sesi aktif" : state === "connecting" ? "Menyambung" : "Tidak aktif"}</Badge>
          <Button onClick={() => term.current?.clear()} disabled={state !== "open"}><Eraser className="size-4" aria-hidden />Clear</Button>
          <Button onClick={() => void copy()}><ClipboardCopy className="size-4" aria-hidden />Copy</Button>
          <Button variant="primary" onClick={() => void open()} disabled={!isConnected}><PlugZap className="size-4" aria-hidden />Reconnect</Button>
        </>} />
      {!active ? (
        <Card><EmptyState title="Belum ada server" action={<Button variant="primary" onClick={() => go("servers")}>Tambah server</Button>} /></Card>
      ) : !isConnected ? (
        <Card><EmptyState icon={<SquareTerminal className="size-8" />} title={`${active.name} belum terhubung`} action={<Button variant="primary" onClick={() => void connect(active.id)}>Hubungkan</Button>} /></Card>
      ) : null}
      <div className={`min-h-[360px] flex-1 overflow-hidden rounded-md border border-line bg-black ${isConnected ? "" : "hidden"}`}>
        <div ref={host} className="h-full w-full" />
      </div>
      {pending && (
        <ConfirmDialog title="Jalankan perintah berisiko?" destructive confirmLabel="Jalankan" onCancel={pending.cancel} onConfirm={pending.send}
          message={<>Perintah berikut berpotensi merusak data atau mengganggu server. Pastikan Anda memahami dampaknya.
            <pre className="mt-3 whitespace-pre-wrap break-all rounded-md border border-line bg-sidebar p-3 font-mono text-[13px]">{pending.text}</pre></>} />
      )}
    </div>
  );
}
