import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Monitor, Moon, Sun, Trash2 } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { AppSettings, KnownHost } from "../../types";
import { Button, Card, ConfirmDialog, EmptyState, Field, Input, PageHeader, Table, Td, Th, Tr, cx } from "../../components/ui";
import { formatDateTime } from "../../lib/format";
import type { ThemeChoice } from "../../lib/theme";

const THEMES: Array<{ id: ThemeChoice; label: string; icon: React.ReactNode }> = [
  { id: "light", label: "Light", icon: <Sun className="size-4" /> },
  { id: "dark", label: "Dark", icon: <Moon className="size-4" /> },
  { id: "system", label: "System", icon: <Monitor className="size-4" /> },
];

export function SettingsPage() {
  const { settings, setSettings, theme, setTheme, notify, fail } = useApp();
  const [s, setS] = useState<AppSettings | null>(settings);
  const [saving, setSaving] = useState(false);
  const [hosts, setHosts] = useState<KnownHost[]>([]);
  const [forget, setForget] = useState<KnownHost | null>(null);

  useEffect(() => { if (!s) api.settingsGet().then((v) => { setS(v); setSettings(v); }).catch((e) => fail(e)); }, [s, setSettings, fail]);
  const loadHosts = () => api.knownHosts().then(setHosts).catch((e) => fail(e));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void loadHosts(); }, []);

  if (!s) return <p className="text-fg2">Memuat pengaturan…</p>;
  const set = <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => setS({ ...s, [k]: v });
  const num = (k: "monitor_interval_secs" | "ssh_timeout_secs" | "max_transfer_mb" | "log_retention_days") => (e: React.ChangeEvent<HTMLInputElement>) => set(k, Number(e.target.value));

  const save = async () => {
    setSaving(true);
    try { await api.settingsSet({ ...s, theme }); setSettings({ ...s, theme }); notify("success", "Pengaturan disimpan."); }
    catch (e) { fail(e, "Gagal menyimpan"); } finally { setSaving(false); }
  };
  const pickDir = async () => { const r = await open({ directory: true, multiple: false, title: "Pilih folder unduhan" }); if (typeof r === "string") set("download_dir", r); };

  return (
    <div className="max-w-4xl space-y-4">
      <PageHeader title="Settings & Security" description="Tema, monitoring, timeout, unduhan, Ollama, retensi log, dan host SSH terpercaya." actions={<Button variant="primary" loading={saving} onClick={() => void save()}>Simpan pengaturan</Button>} />

      <Card title="Tampilan">
        <div role="radiogroup" aria-label="Tema" className="flex gap-2">
          {THEMES.map((t) => (
            <button key={t.id} type="button" role="radio" aria-checked={theme === t.id} onClick={() => setTheme(t.id)}
              className={cx("flex h-9 items-center gap-2 rounded-md border px-4", theme === t.id ? "border-fg bg-fg text-bg" : "border-line hover:bg-hover")}>{t.icon}{t.label}</button>))}
        </div>
        <p className="mt-2 text-xs text-fg2">Pilihan tema disimpan secara lokal di perangkat ini. “System” mengikuti preferensi Windows.</p>
      </Card>

      <Card title="Koneksi & monitoring">
        <div className="grid grid-cols-2 gap-4">
          <Field label="Interval monitoring (detik)" hint="3–3600">{(id) => <Input id={id} type="number" min={3} max={3600} value={s.monitor_interval_secs} onChange={num("monitor_interval_secs")} />}</Field>
          <Field label="Timeout SSH (detik)" hint="3–120">{(id) => <Input id={id} type="number" min={3} max={120} value={s.ssh_timeout_secs} onChange={num("ssh_timeout_secs")} />}</Field>
          <Field label="Batas ukuran transfer (MB)">{(id) => <Input id={id} type="number" min={1} value={s.max_transfer_mb} onChange={num("max_transfer_mb")} />}</Field>
          <Field label="Retensi log aktivitas (hari)">{(id) => <Input id={id} type="number" min={1} value={s.log_retention_days} onChange={num("log_retention_days")} />}</Field>
          <div className="col-span-2"><Field label="Folder unduhan">{(id) => (
            <div className="flex gap-2"><Input id={id} value={s.download_dir} onChange={(e) => set("download_dir", e.target.value)} /><Button onClick={() => void pickDir()}><FolderOpen className="size-4" aria-hidden />Pilih…</Button></div>)}</Field></div>
        </div>
      </Card>

      <Card title="Ollama (AI opsional)">
        <div className="grid grid-cols-2 gap-4">
          <Field label="URL Ollama" hint="Hanya alamat lokal (127.0.0.1 / localhost) yang diizinkan.">{(id) => <Input id={id} value={s.ollama_url} onChange={(e) => set("ollama_url", e.target.value)} />}</Field>
          <Field label="Model">{(id) => <Input id={id} value={s.ollama_model} onChange={(e) => set("ollama_model", e.target.value)} />}</Field>
        </div>
      </Card>

      <Card title="Host SSH terpercaya" pad={false}>
        {hosts.length === 0 ? <EmptyState title="Belum ada host terpercaya" description="Host key disimpan setelah Anda menyetujui fingerprint saat pertama terhubung." /> : (
          <Table>
            <thead><tr><Th>Host</Th><Th>Tipe</Th><Th>Fingerprint</Th><Th>Disetujui</Th><Th className="text-right">Aksi</Th></tr></thead>
            <tbody>{hosts.map((h) => (
              <Tr key={`${h.host}:${h.port}`}><Td className="font-mono text-[13px]">{h.host}:{h.port}</Td><Td>{h.key_type}</Td><Td className="max-w-72 break-all font-mono text-xs">{h.fingerprint}</Td><Td className="text-fg2">{formatDateTime(h.trusted_at)}</Td>
                <Td className="text-right"><Button size="sm" onClick={() => setForget(h)}><Trash2 className="size-3.5" aria-hidden />Lupakan</Button></Td></Tr>))}</tbody>
          </Table>)}
      </Card>
      <p className="text-xs text-fg2">Keamanan: password dan passphrase disimpan di Windows Credential Manager; private key hanya dibaca dari path yang Anda pilih; verifikasi host key tidak dapat dimatikan.</p>
      {forget && <ConfirmDialog title="Lupakan host key?" destructive confirmLabel="Lupakan" onCancel={() => setForget(null)} message={<>Host key <b>{forget.host}:{forget.port}</b> dihapus. Koneksi berikutnya akan meminta Anda memverifikasi fingerprint lagi.</>}
        onConfirm={async () => { try { await api.forgetHost(forget.host, forget.port); setForget(null); void loadHosts(); } catch (e) { fail(e); setForget(null); } }} />}
    </div>
  );
}
