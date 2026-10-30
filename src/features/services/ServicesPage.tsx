import { useState } from "react";
import { RefreshCw, ScrollText } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { Overview, ProcessInfo, ServiceInfo } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { ServerGate } from "../../components/ServerGate";
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, ErrorState, Input, Modal, PageHeader, Spinner, Table, Tabs, Td, Th, Tr } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import type { Tone } from "../../components/ui";

const tone = (s: ServiceInfo): Tone => (s.load !== "loaded" ? "neutral" : s.active === "active" ? "ok" : s.active === "failed" ? "error" : s.active === "activating" || s.active === "reloading" ? "active" : "warn");
const label = (s: ServiceInfo) => (s.load !== "loaded" ? "Tidak ada" : s.active === "active" ? `Aktif (${s.sub})` : s.active === "failed" ? "Gagal" : `${s.active} (${s.sub})`);

function Summary({ ov }: { ov: Overview }) {
  const find = (n: string | null) => ov.services.find((s) => s.name === n);
  const items: Array<{ title: string; names: Array<string | null> }> = [
    { title: "Nginx", names: [ov.detected.nginx] },
    { title: "Database", names: [ov.detected.database] },
    { title: "PHP-FPM", names: ov.detected.php_fpm.length ? ov.detected.php_fpm : [null] },
    { title: "SSH", names: [ov.detected.ssh] },
  ];
  return (
    <div className="mb-4 grid grid-cols-4 gap-4">
      {items.map((it) => (
        <div key={it.title} className="rounded-md border border-line bg-card p-4">
          <div className="mb-2 text-[13px] text-fg2">{it.title}</div>
          {it.names.map((n, i) => {
            const s = find(n);
            return (
              <div key={i} className="flex items-center justify-between gap-2">
                <span className="truncate font-mono text-[13px]">{n ?? "tidak terdeteksi"}</span>
                {s ? <Badge tone={tone(s)}>{s.active}</Badge> : <Badge>—</Badge>}
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}

function Content({ serverId }: { serverId: number }) {
  const { notify, fail } = useApp();
  const [tab, setTab] = useState<"services" | "processes" | "pm2">("services");
  const ov = useAsync<Overview>(() => api.servicesOverview(serverId), [serverId]);
  const ps = useAsync<ProcessInfo[]>(() => (tab === "processes" ? api.processes(serverId) : Promise.resolve([])), [serverId, tab]);
  const [q, setQ] = useState("");
  const [onlyActive, setOnlyActive] = useState(true);
  const [ask, setAsk] = useState<{ unit: string; action: "restart" | "reload" } | null>(null);
  const [logs, setLogs] = useState<{ unit: string; text: string | null; err?: string } | null>(null);

  const openLogs = async (unit: string) => {
    setLogs({ unit, text: null });
    try { setLogs({ unit, text: await api.serviceLogs(serverId, unit, 200) }); }
    catch (e) { setLogs({ unit, text: "", err: (e as { message: string }).message }); }
  };

  const list = (ov.data?.services ?? []).filter((s) => (!onlyActive || s.active === "active" || s.active === "failed") && s.load === "loaded" && (!q || s.name.toLowerCase().includes(q.toLowerCase())));

  return (
    <div>
      <PageHeader title="Services" description="Status layanan systemd, proses, dan PM2. Restart/reload selalu meminta konfirmasi dan diverifikasi sesudahnya."
        actions={<Button onClick={() => { void ov.reload(); void ps.reload(); }}><RefreshCw className="size-4" aria-hidden />Muat ulang</Button>} />
      {ov.data && <Summary ov={ov.data} />}
      <Tabs value={tab} onChange={setTab} tabs={[{ id: "services", label: "Layanan" }, { id: "processes", label: "Proses" }, { id: "pm2", label: "PM2" }]} />

      {tab === "services" && (
        <Card pad={false} title="Layanan systemd" actions={<>
          <Checkbox checked={onlyActive} onChange={setOnlyActive} label="Hanya aktif/gagal" />
          <Input aria-label="Cari layanan" placeholder="Cari layanan…" className="h-8 w-48" value={q} onChange={(e) => setQ(e.target.value)} />
        </>}>
          {ov.loading && !ov.data ? <Spinner /> : ov.error ? <ErrorState message={ov.error} onRetry={() => void ov.reload()} /> : list.length === 0 ? <EmptyState title="Tidak ada layanan yang cocok" /> : (
            <Table>
              <thead><tr><Th>Layanan</Th><Th>Status</Th><Th>Deskripsi</Th><Th className="text-right">Aksi</Th></tr></thead>
              <tbody>{list.map((s) => (
                <Tr key={s.name}>
                  <Td className="font-mono text-[13px]">{s.name}</Td>
                  <Td><Badge tone={tone(s)}>{label(s)}</Badge></Td>
                  <Td className="max-w-80 truncate text-fg2" title={s.description}>{s.description}</Td>
                  <Td className="text-right"><div className="flex justify-end gap-1">
                    <Button size="sm" variant="ghost" onClick={() => void openLogs(s.name)}><ScrollText className="size-3.5" aria-hidden />Log</Button>
                    <Button size="sm" onClick={() => setAsk({ unit: s.name, action: "reload" })}>Reload</Button>
                    <Button size="sm" onClick={() => setAsk({ unit: s.name, action: "restart" })}>Restart</Button>
                  </div></Td>
                </Tr>))}</tbody>
            </Table>
          )}
        </Card>
      )}

      {tab === "processes" && (
        <Card pad={false} title="40 proses teratas berdasarkan CPU">
          {ps.loading && !ps.data ? <Spinner /> : ps.error ? <ErrorState message={ps.error} onRetry={() => void ps.reload()} /> : (
            <Table>
              <thead><tr><Th>PID</Th><Th>User</Th><Th>CPU %</Th><Th>MEM %</Th><Th>RSS</Th><Th>Waktu</Th><Th>Perintah</Th></tr></thead>
              <tbody>{(ps.data ?? []).map((p) => (
                <Tr key={p.pid}><Td className="tabular-nums">{p.pid}</Td><Td>{p.user}</Td><Td className="tabular-nums">{p.cpu.toFixed(1)}</Td><Td className="tabular-nums">{p.mem.toFixed(1)}</Td>
                  <Td className="tabular-nums">{formatBytes(p.rss_kb * 1024)}</Td><Td>{p.elapsed}</Td><Td className="max-w-96 truncate font-mono text-[13px]" title={p.command}>{p.command}</Td></Tr>))}</tbody>
            </Table>
          )}
        </Card>
      )}

      {tab === "pm2" && (
        <Card pad={false} title="Proses PM2">
          {!ov.data ? <Spinner /> : !ov.data.pm2_available ? <EmptyState title="PM2 tidak terdeteksi" description="PM2 tidak terpasang untuk user SSH ini, atau tidak ada di PATH." /> :
            ov.data.pm2.length === 0 ? <EmptyState title="Tidak ada proses PM2" /> : (
            <Table>
              <thead><tr><Th>Nama</Th><Th>Status</Th><Th>PID</Th><Th>CPU %</Th><Th>Memori</Th><Th>Restart</Th></tr></thead>
              <tbody>{ov.data.pm2.map((p) => (
                <Tr key={p.name}><Td className="font-medium">{p.name}</Td><Td><Badge tone={p.status === "online" ? "ok" : p.status === "errored" ? "error" : "warn"}>{p.status}</Badge></Td>
                  <Td>{p.pid ?? "—"}</Td><Td>{p.cpu?.toFixed(1) ?? "—"}</Td><Td>{formatBytes(p.memory)}</Td><Td>{p.restarts ?? "—"}</Td></Tr>))}</tbody>
            </Table>
          )}
        </Card>
      )}

      {ask && (
        <ConfirmDialog title={`${ask.action === "restart" ? "Restart" : "Reload"} ${ask.unit}?`} destructive={ask.action === "restart"} confirmLabel={ask.action === "restart" ? "Restart" : "Reload"} onCancel={() => setAsk(null)}
          message={ask.action === "restart" ? "Restart akan menghentikan layanan sesaat dan dapat memutus koneksi yang sedang berjalan." : "Reload menerapkan konfigurasi tanpa menghentikan layanan (jika didukung)."}
          onConfirm={async () => {
            try {
              const r = await api.serviceAction(serverId, ask.unit, ask.action, true);
              notify(r.exit === 0 && ["active", "reloading"].includes(r.active_after) ? "success" : "error", r.message);
              setAsk(null); void ov.reload();
            } catch (e) { fail(e); setAsk(null); }
          }} />
      )}
      {logs && (
        <Modal title={`Log: ${logs.unit}`} onClose={() => setLogs(null)} wide footer={<Button onClick={() => setLogs(null)}>Tutup</Button>}>
          {logs.text === null ? <Spinner /> : logs.err ? <ErrorState message={logs.err} /> :
            <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded-md border border-line bg-sidebar p-3 font-mono text-xs">{logs.text || "(log kosong)"}</pre>}
        </Modal>
      )}
    </div>
  );
}

export function ServicesPage() {
  return <ServerGate>{(id) => <Content serverId={id} />}</ServerGate>;
}
