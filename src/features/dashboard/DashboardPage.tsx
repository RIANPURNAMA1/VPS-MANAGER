import { useEffect, useRef, useState } from "react";
import { Activity as ActivityIcon, Clock, Cpu, HardDrive, MemoryStick, Network, Server } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { LogEntry, Metrics } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { Badge, Button, Card, EmptyState, ErrorState, PageHeader, Table, Td, Th, Tr } from "../../components/ui";
import { LineChart, Meter } from "../../components/Sparkline";
import { formatBytes, formatDateTime, formatPercent, formatRate, formatUptime, timeAgo } from "../../lib/format";

const HISTORY = 40;
type Hist = { cpu: Array<number | null>; mem: Array<number | null>; rx: Array<number | null>; tx: Array<number | null> };

function Stat({ icon, label, value, sub, meter }: { icon: React.ReactNode; label: string; value: string; sub?: string; meter?: number }) {
  return (
    <div className="rounded-md border border-line bg-card p-4">
      <div className="mb-2 flex items-center gap-2 text-fg2">{icon}<span className="text-[13px]">{label}</span></div>
      <div className="text-2xl font-semibold tabular-nums">{value}</div>
      {sub && <div className="mt-0.5 text-xs text-fg2">{sub}</div>}
      {meter != null && <div className="mt-3"><Meter value={meter} label={label} /></div>}
    </div>
  );
}

function Monitoring({ serverId }: { serverId: number }) {
  const { settings } = useApp();
  const interval = (settings?.monitor_interval_secs ?? 10) * 1000;
  const [hist, setHist] = useState<Hist>({ cpu: [], mem: [], rx: [], tx: [] });
  const lastServer = useRef(serverId);
  const { data: m, error, loading, updatedAt, reload } = useAsync<Metrics>(() => api.metrics(serverId), [serverId], interval);
  const [now, setNow] = useState(Date.now());
  useEffect(() => { const t = window.setInterval(() => setNow(Date.now()), 1000); return () => window.clearInterval(t); }, []);

  useEffect(() => {
    if (!m) return;
    setHist((h) => {
      const base: Hist = lastServer.current === serverId ? h : { cpu: [], mem: [], rx: [], tx: [] };
      lastServer.current = serverId;
      const push = (a: Array<number | null>, v: number | null) => [...a, v].slice(-HISTORY);
      return { cpu: push(base.cpu, m.cpu_percent), mem: push(base.mem, m.mem_total ? (m.mem_used / m.mem_total) * 100 : null), rx: push(base.rx, m.net_rx_bps), tx: push(base.tx, m.net_tx_bps) };
    });
  }, [m, serverId]);

  if (loading && !m) return <Card><div className="p-10 text-center text-fg2">Mengambil metrik dari server…</div></Card>;
  if (!m) return <Card><ErrorState message={error ?? "Tidak ada data."} onRetry={() => void reload()} title="Gagal membaca metrik" /></Card>;

  const stale = updatedAt != null && now - updatedAt > interval * 3;
  const memPct = m.mem_total ? (m.mem_used / m.mem_total) * 100 : 0;
  const root = m.disks.find((d) => d.mount === "/") ?? m.disks[0];
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3 text-[13px] text-fg2">
        <span>{m.hostname} · {m.os} · kernel {m.kernel}</span>
        <span className="ml-auto">Diperbarui {updatedAt ? timeAgo(updatedAt, now) : "—"}</span>
        {error ? <Badge tone="error" className="">Gagal memperbarui</Badge> : stale ? <Badge tone="warn">Data usang</Badge> : <Badge tone="ok">Terkini</Badge>}
      </div>
      {error && <p className="rounded-md border border-dashed border-fg p-2 text-[13px]">Pembaruan terakhir gagal: {error}. Menampilkan data sebelumnya.</p>}
      <div className="grid grid-cols-4 gap-4">
        <Stat icon={<Cpu className="size-4" />} label="CPU" value={formatPercent(m.cpu_percent, 1)} sub={`${m.cpu_count} core · load ${m.load.map((x) => x.toFixed(2)).join(" ")}`} meter={m.cpu_percent ?? 0} />
        <Stat icon={<MemoryStick className="size-4" />} label="RAM" value={formatPercent(memPct, 0)} sub={`${formatBytes(m.mem_used)} dari ${formatBytes(m.mem_total)} · tersedia ${formatBytes(m.mem_available)}`} meter={memPct} />
        <Stat icon={<HardDrive className="size-4" />} label={`Disk ${root?.mount ?? ""}`} value={root ? formatPercent(root.used_percent, 0) : "—"} sub={root ? `${formatBytes(root.used_bytes)} dari ${formatBytes(root.total_bytes)}` : "Tidak terdeteksi"} meter={root?.used_percent ?? 0} />
        <Stat icon={<Clock className="size-4" />} label="Uptime" value={formatUptime(m.uptime_secs)} sub={m.swap_total ? `Swap ${formatBytes(m.swap_used)} / ${formatBytes(m.swap_total)}` : "Tanpa swap"} />
      </div>
      <div className="grid grid-cols-3 gap-4">
        <Card title="CPU (%)"><LineChart label="Riwayat CPU" series={[{ name: "CPU", values: hist.cpu }]} max={100} /></Card>
        <Card title="RAM (%)"><LineChart label="Riwayat RAM" series={[{ name: "RAM", values: hist.mem }]} max={100} /></Card>
        <Card title="Lalu lintas jaringan" actions={<span className="text-xs text-fg2"><Network className="mr-1 inline size-3.5" />↓ {formatRate(m.net_rx_bps)} · ↑ {formatRate(m.net_tx_bps)}</span>}>
          <LineChart label="Riwayat jaringan" unit=" B/s" series={[{ name: "Masuk", values: hist.rx }, { name: "Keluar", values: hist.tx, dashed: true }]} />
        </Card>
      </div>
      {m.disks.length > 1 && (
        <Card title="Penggunaan disk" pad={false}>
          <Table>
            <thead><tr><Th>Mount</Th><Th>Filesystem</Th><Th>Tipe</Th><Th>Terpakai</Th><Th>Total</Th><Th className="w-48">Penggunaan</Th></tr></thead>
            <tbody>{m.disks.map((d) => (
              <Tr key={d.mount}><Td className="font-mono text-[13px]">{d.mount}</Td><Td className="font-mono text-[13px]">{d.filesystem}</Td><Td>{d.fstype}</Td>
                <Td>{formatBytes(d.used_bytes)}</Td><Td>{formatBytes(d.total_bytes)}</Td>
                <Td><div className="flex items-center gap-2"><div className="flex-1"><Meter value={d.used_percent} label={`Disk ${d.mount}`} /></div><span className="w-10 text-right tabular-nums">{formatPercent(d.used_percent)}</span></div></Td></Tr>
            ))}</tbody>
          </Table>
        </Card>
      )}
    </div>
  );
}

export function DashboardPage() {
  const { profiles, connected, active, isConnected, connect, go, setActiveId } = useApp();
  const { data: logs } = useAsync<LogEntry[]>(() => api.logs({ limit: 8 }), []);

  return (
    <div className="space-y-4">
      <PageHeader title="Overview" description="Ringkasan server, kondisi sumber daya, dan aktivitas terbaru. Semua angka berasal dari server nyata." />
      <div className="grid grid-cols-3 gap-4">
        <Stat icon={<Server className="size-4" />} label="Jumlah server" value={String(profiles.length)} sub={`${connected.size} terhubung`} />
        <Stat icon={<ActivityIcon className="size-4" />} label="Status koneksi SSH" value={isConnected ? "Terhubung" : "Terputus"} sub={active ? `${active.username}@${active.host}:${active.port}` : "Belum ada server aktif"} />
        <Stat icon={<Network className="size-4" />} label="Server aktif" value={active?.name ?? "—"} sub={active ? (active.use_sudo ? "sudo -n aktif" : "Tanpa sudo") : undefined} />
      </div>

      {!active ? (
        <Card><EmptyState title="Belum ada data monitoring" description="Tambahkan server dan hubungkan untuk melihat CPU, RAM, disk, uptime, dan lalu lintas jaringan." action={<Button variant="primary" onClick={() => go("servers")}>Tambah server</Button>} /></Card>
      ) : !isConnected ? (
        <Card><EmptyState title={`${active.name} belum terhubung`} description="Monitoring hanya tampil dari server yang sedang terhubung. Tidak ada angka contoh yang ditampilkan." action={<Button variant="primary" onClick={() => void connect(active.id)}>Hubungkan</Button>} /></Card>
      ) : (
        <Monitoring key={active.id} serverId={active.id} />
      )}

      <div className="grid grid-cols-2 gap-4">
        <Card title="Server" pad={false}>
          {profiles.length === 0 ? <EmptyState title="Belum ada server" /> : (
            <Table>
              <thead><tr><Th>Nama</Th><Th>Host</Th><Th>Status</Th></tr></thead>
              <tbody>{profiles.map((p) => (
                <Tr key={p.id} onClick={() => setActiveId(p.id)}>
                  <Td className="font-medium">{p.name}</Td><Td className="font-mono text-[13px]">{p.host}</Td>
                  <Td><Badge tone={connected.has(p.id) ? "ok" : "neutral"}>{connected.has(p.id) ? "Terhubung" : "Terputus"}</Badge></Td>
                </Tr>))}</tbody>
            </Table>
          )}
        </Card>
        <Card title="Aktivitas terbaru" actions={<Button size="sm" variant="ghost" onClick={() => go("logs")}>Lihat semua</Button>} pad={false}>
          {!logs || logs.length === 0 ? <EmptyState title="Belum ada aktivitas" /> : (
            <ul className="divide-y divide-line">
              {logs.map((l) => (
                <li key={l.id} className="flex items-center gap-3 px-4 py-2">
                  <Badge tone={l.status === "success" ? "ok" : "error"}>{l.status === "success" ? "Sukses" : l.status === "blocked" ? "Diblokir" : "Gagal"}</Badge>
                  <span className="min-w-0 flex-1 truncate" title={l.summary ?? ""}><span className="font-mono text-[13px]">{l.action}</span> <span className="text-fg2">{l.summary}</span></span>
                  <span className="shrink-0 text-xs text-fg3">{formatDateTime(l.ts)}</span>
                </li>))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}
