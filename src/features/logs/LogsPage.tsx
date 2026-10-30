import { useState } from "react";
import { RefreshCw, Trash2 } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { LogEntry } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { Badge, Button, Card, ConfirmDialog, EmptyState, ErrorState, Input, PageHeader, Select, Spinner, Table, Td, Th, Tr } from "../../components/ui";
import { formatDateTime, formatDuration } from "../../lib/format";

export function LogsPage() {
  const { fail, notify } = useApp();
  const [status, setStatus] = useState("");
  const [search, setSearch] = useState("");
  const [q, setQ] = useState("");
  const [ask, setAsk] = useState(false);
  const { data, error, loading, reload } = useAsync<LogEntry[]>(() => api.logs({ status, search: q, limit: 300 }), [status, q]);
  const label = (s: string) => (s === "success" ? "Sukses" : s === "blocked" ? "Diblokir" : "Gagal");

  return (
    <div>
      <PageHeader title="Activity Logs" description="Riwayat tindakan, hasil, exit code, dan durasi. Ringkasan disamarkan; password, key, token, dan isi database tidak pernah dicatat."
        actions={<><Button onClick={() => void reload()}><RefreshCw className="size-4" aria-hidden />Muat ulang</Button><Button variant="danger" onClick={() => setAsk(true)}><Trash2 className="size-4" aria-hidden />Hapus semua</Button></>} />
      <form className="mb-3 flex gap-2" onSubmit={(e) => { e.preventDefault(); setQ(search); }}>
        <Input aria-label="Cari log" placeholder="Cari aksi, server, atau ringkasan…" value={search} onChange={(e) => setSearch(e.target.value)} className="max-w-sm" />
        <Select aria-label="Filter status" value={status} onChange={(e) => setStatus(e.target.value)} className="w-40">
          <option value="">Semua status</option><option value="success">Sukses</option><option value="error">Gagal</option><option value="blocked">Diblokir</option>
        </Select>
        <Button type="submit">Cari</Button>
      </form>
      <Card pad={false}>
        {loading && !data ? <Spinner /> : error ? <ErrorState message={error} onRetry={() => void reload()} /> : !data || data.length === 0 ? <EmptyState title="Belum ada aktivitas" description="Tindakan seperti koneksi, restart service, backup, dan deploy akan tercatat di sini." /> : (
          <Table>
            <thead><tr><Th>Waktu</Th><Th>Server</Th><Th>Aksi</Th><Th>Status</Th><Th>Exit</Th><Th>Durasi</Th><Th>Ringkasan</Th></tr></thead>
            <tbody>{data.map((l) => (
              <Tr key={l.id}>
                <Td className="whitespace-nowrap text-fg2">{formatDateTime(l.ts)}</Td>
                <Td>{l.server_name ?? "—"}</Td>
                <Td className="font-mono text-[13px]">{l.action}</Td>
                <Td><Badge tone={l.status === "success" ? "ok" : "error"}>{label(l.status)}</Badge></Td>
                <Td className="tabular-nums">{l.exit_code ?? "—"}</Td>
                <Td className="whitespace-nowrap tabular-nums">{formatDuration(l.duration_ms)}</Td>
                <Td className="max-w-md truncate text-fg2" title={l.summary ?? ""}>{l.summary}</Td>
              </Tr>))}</tbody>
          </Table>)}
      </Card>
      {ask && <ConfirmDialog title="Hapus semua log aktivitas?" destructive confirmLabel="Hapus semua" onCancel={() => setAsk(false)} message="Seluruh riwayat aktivitas lokal akan dihapus permanen. Riwayat backup dan deployment tidak terpengaruh."
        onConfirm={async () => { try { const n = await api.logsClear(); notify("success", `${n} log dihapus.`); setAsk(false); void reload(); } catch (e) { fail(e); setAsk(false); } }} />}
    </div>
  );
}
