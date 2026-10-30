import { useEffect, useState } from "react";
import { Database, Download, RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { BackupBatch, BackupRecord, DbInfo, RemoteBackup } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { ServerGate } from "../../components/ServerGate";
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, ErrorState, Field, IconButton, Input, PageHeader, Select, Spinner, Table, Tabs, Td, Th, Tr } from "../../components/ui";
import { formatBytes, formatDateTime, shortHash } from "../../lib/format";

type Restore = { file: string; db: string };

function Content({ serverId }: { serverId: number }) {
  const { notify, fail } = useApp();
  const [tab, setTab] = useState<"db" | "history">("db");
  const dbs = useAsync<DbInfo[]>(() => api.dbList(serverId), [serverId]);
  const hist = useAsync<BackupRecord[]>(() => api.backupHistory(serverId), [serverId]);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [dir, setDir] = useState("");
  const [dl, setDl] = useState(true);
  const [busy, setBusy] = useState(false);
  const [batch, setBatch] = useState<BackupBatch | null>(null);
  const [remote, setRemote] = useState<RemoteBackup[] | null>(null);
  const [restore, setRestore] = useState<Restore | null>(null);
  const [target, setTarget] = useState("");
  const [rm, setRm] = useState<BackupRecord | null>(null);

  useEffect(() => { api.backupDefaultDir(serverId).then(setDir).catch(() => undefined); }, [serverId]);

  const create = async () => {
    setBusy(true); setBatch(null);
    try {
      const b = await api.backupCreate(serverId, [...sel], dir, dl);
      setBatch(b); void hist.reload(); void dbs.reload();
      notify(b.failed ? "error" : "success", b.failed ? `${b.succeeded} berhasil, ${b.failed} gagal/parsial.` : `${b.succeeded} backup berhasil dan terverifikasi.`);
    } catch (e) { fail(e, "Backup gagal"); } finally { setBusy(false); }
  };
  const loadRemote = async () => { try { setRemote(await api.backupRemoteList(serverId, dir)); } catch (e) { fail(e); } };
  const tone = (s: string) => (s === "success" ? "ok" : s === "partial" ? "warn" : "error");
  const lbl = (s: string) => (s === "success" ? "Berhasil" : s === "partial" ? "Parsial" : "Gagal");

  const userDbs = (dbs.data ?? []).filter((d) => !d.is_system);

  return (
    <div>
      <PageHeader title="Databases & Backups" description="Backup MySQL/MariaDB lewat mysqldump di server, diverifikasi (gzip -t, penanda Dump completed, checksum), lalu opsional diunduh ke Windows."
        actions={<Button onClick={() => { void dbs.reload(); void hist.reload(); }}><RefreshCw className="size-4" aria-hidden />Muat ulang</Button>} />
      <Tabs value={tab} onChange={setTab} tabs={[{ id: "db", label: "Database & backup" }, { id: "history", label: "Riwayat & restore" }]} />

      {tab === "db" && (
        <div className="grid grid-cols-5 gap-4">
          <Card className="col-span-3" title="Database yang dapat diakses" pad={false}>
            {dbs.loading && !dbs.data ? <Spinner /> : dbs.error ? <ErrorState message={dbs.error} onRetry={() => void dbs.reload()} title="Tidak dapat membaca database" /> :
              userDbs.length === 0 ? <EmptyState icon={<Database className="size-8" />} title="Tidak ada database pengguna" /> : (
              <Table>
                <thead><tr><Th className="w-10" /><Th>Database</Th><Th>Ukuran</Th></tr></thead>
                <tbody>{userDbs.map((d) => (
                  <Tr key={d.name}>
                    <Td><input type="checkbox" aria-label={`Pilih ${d.name}`} className="size-4 accent-[var(--fg)]" checked={sel.has(d.name)}
                      onChange={(e) => setSel((s) => { const n = new Set(s); if (e.target.checked) n.add(d.name); else n.delete(d.name); return n; })} /></Td>
                    <Td className="font-mono text-[13px]">{d.name}</Td><Td>{formatBytes(d.size_bytes)}</Td>
                  </Tr>))}</tbody>
              </Table>
            )}
          </Card>
          <Card className="col-span-2" title="Buat backup">
            <div className="space-y-4">
              <Field label="Lokasi backup di server" hint="Path absolut. File memakai nama berstempel waktu dan tidak pernah menimpa backup lama.">{(id) => <Input id={id} className="font-mono text-[13px]" value={dir} onChange={(e) => setDir(e.target.value)} />}</Field>
              <Checkbox checked={dl} onChange={setDl} label="Unduh ke Windows setelah terverifikasi" />
              <Button variant="primary" className="w-full" loading={busy} disabled={sel.size === 0 || !dir} onClick={() => void create()}>Backup {sel.size || ""} database</Button>
              {batch && (
                <ul className="space-y-2 border-t border-line pt-3">
                  {batch.records.map((r) => (
                    <li key={r.id} className="text-[13px]">
                      <div className="flex items-center justify-between gap-2"><span className="font-mono">{r.database_name}</span><Badge tone={tone(r.status)}>{lbl(r.status)}</Badge></div>
                      {r.note && <p className="mt-1 text-fg2">{r.note}</p>}
                      {r.verified && <p className="mt-1 text-fg2">{formatBytes(r.size_bytes)} · sha256 {shortHash(r.sha256, 12)}</p>}
                    </li>))}
                </ul>
              )}
            </div>
          </Card>
        </div>
      )}

      {tab === "history" && (
        <div className="space-y-4">
          <Card title="Riwayat backup" pad={false}>
            {hist.loading && !hist.data ? <Spinner /> : !hist.data || hist.data.length === 0 ? <EmptyState title="Belum ada backup" description="Backup yang dibuat dari aplikasi ini akan tercatat di sini." /> : (
              <Table>
                <thead><tr><Th>Waktu</Th><Th>Database</Th><Th>Status</Th><Th>Ukuran</Th><Th>Verifikasi</Th><Th>Lokasi</Th><Th className="text-right">Aksi</Th></tr></thead>
                <tbody>{hist.data.map((b) => (
                  <Tr key={b.id}>
                    <Td className="whitespace-nowrap text-fg2">{formatDateTime(b.created_at)}</Td>
                    <Td className="font-mono text-[13px]">{b.database_name}</Td>
                    <Td><Badge tone={tone(b.status)}>{lbl(b.status)}</Badge></Td>
                    <Td>{formatBytes(b.size_bytes)}</Td>
                    <Td>{b.verified ? <span title={b.sha256 ?? ""}>sha256 {shortHash(b.sha256, 8)}</span> : <span className="text-fg3">—</span>}</Td>
                    <Td className="max-w-72 text-xs text-fg2"><div className="truncate font-mono" title={b.remote_path}>Server: {b.remote_path}</div>{b.local_path && <div className="truncate font-mono" title={b.local_path}>Lokal: {b.local_path}</div>}{b.note && <div className="text-fg">{b.note}</div>}</Td>
                    <Td className="text-right"><div className="flex justify-end">
                      {b.verified && !b.local_path && <IconButton label="Unduh ke Windows" onClick={async () => { try { const p = await api.backupDownload(serverId, b.id); notify("success", `Tersimpan di ${p}`); void hist.reload(); } catch (e) { fail(e, "Unduh gagal"); } }}><Download className="size-4" /></IconButton>}
                      {b.verified && <IconButton label="Restore backup ini" onClick={() => { setRestore({ file: b.remote_path, db: b.database_name }); setTarget(b.database_name); }}><RotateCcw className="size-4" /></IconButton>}
                      <IconButton label="Hapus catatan (file di server tidak dihapus)" onClick={() => setRm(b)}><Trash2 className="size-4" /></IconButton>
                    </div></Td>
                  </Tr>))}</tbody>
              </Table>
            )}
          </Card>
          <Card title="File backup di server" actions={<Button size="sm" onClick={() => void loadRemote()}>Pindai {dir}</Button>} pad={false}>
            {!remote ? <EmptyState title="Belum dipindai" description="Pindai lokasi backup untuk melihat file .sql.gz yang ada, termasuk yang dibuat di luar aplikasi." /> : remote.length === 0 ? <EmptyState title="Tidak ada file .sql.gz" /> : (
              <Table>
                <thead><tr><Th>File</Th><Th>Ukuran</Th><Th>Diubah</Th><Th className="text-right">Aksi</Th></tr></thead>
                <tbody>{remote.map((r) => (
                  <Tr key={r.path}><Td className="font-mono text-[13px]">{r.path}</Td><Td>{formatBytes(r.size_bytes)}</Td><Td className="text-fg2">{formatDateTime(r.modified)}</Td>
                    <Td className="text-right"><Button size="sm" onClick={() => { setRestore({ file: r.path, db: "" }); setTarget(""); }}><RotateCcw className="size-3.5" aria-hidden />Restore</Button></Td></Tr>))}</tbody>
              </Table>
            )}
          </Card>
        </div>
      )}

      {restore && (
        <ConfirmDialog title="Restore database" destructive confirmLabel="Restore sekarang" requireText={target || undefined} onCancel={() => setRestore(null)}
          message={<>Isi database tujuan akan <b>ditimpa</b> oleh isi file berikut. Aksi ini tidak dapat dibatalkan. Disarankan membuat backup baru terlebih dahulu.
            <pre className="mt-2 break-all rounded-md border border-line bg-sidebar p-2 font-mono text-[13px]">{restore.file}</pre></>}
          onConfirm={async () => {
            if (!target) { notify("error", "Pilih database tujuan."); return; }
            try { const m = await api.backupRestore(serverId, restore.file, target, target); notify("success", m); setRestore(null); void dbs.reload(); }
            catch (e) { fail(e, "Restore gagal"); setRestore(null); }
          }}>
          <Field label="Database tujuan (harus sudah ada)">{(id) => (
            <Select id={id} value={target} onChange={(e) => setTarget(e.target.value)}>
              <option value="">— pilih —</option>{userDbs.map((d) => <option key={d.name} value={d.name}>{d.name}</option>)}
            </Select>)}</Field>
        </ConfirmDialog>
      )}
      {rm && (
        <ConfirmDialog title="Hapus catatan backup?" confirmLabel="Hapus catatan" onCancel={() => setRm(null)}
          message="Hanya catatan di aplikasi yang dihapus. File backup di server dan di Windows tidak disentuh."
          onConfirm={async () => { try { await api.backupDeleteRecord(rm.id); setRm(null); void hist.reload(); } catch (e) { fail(e); setRm(null); } }} />
      )}
    </div>
  );
}

export function DatabasesPage() {
  return <ServerGate>{(id) => <Content serverId={id} />}</ServerGate>;
}
