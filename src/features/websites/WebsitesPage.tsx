import { useState } from "react";
import { FileCode, Globe, RefreshCw, ShieldCheck, ShieldQuestion, Stethoscope } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api, toAppErr } from "../../lib/api";
import type { CertInfo, Site, UrlCheck } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { ServerGate } from "../../components/ServerGate";
import { Badge, Button, Card, ConfirmDialog, EmptyState, ErrorState, Modal, PageHeader, Spinner, Table, Td, Th, Tr } from "../../components/ui";

type Check = { http?: UrlCheck; https?: UrlCheck; cert?: CertInfo | { error: string } };

function primaryDomain(s: Site): string | null {
  return s.server_names.find((n) => n !== "_" && !n.includes("*") && !n.startsWith("~")) ?? null;
}

function UrlBadge({ c }: { c?: UrlCheck }) {
  if (!c) return <span className="text-fg3">—</span>;
  if (c.status != null) return <Badge tone={c.ok ? "ok" : "error"}>{c.status} · {c.elapsed_ms} ms</Badge>;
  return <span title={c.error ?? ""}><Badge tone="error">Gagal</Badge></span>;
}

function Content({ serverId }: { serverId: number }) {
  const { notify, fail } = useApp();
  const { data, error, loading, reload } = useAsync<Site[]>(() => api.websites(serverId), [serverId]);
  const [checks, setChecks] = useState<Record<string, Check>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [view, setView] = useState<Site | null>(null);
  const [nginx, setNginx] = useState<{ ok: boolean; output: string } | null>(null);
  const [reloadAsk, setReloadAsk] = useState(false);
  const [testing, setTesting] = useState(false);

  const check = async (s: Site) => {
    const d = primaryDomain(s);
    if (!d) { notify("info", "Situs ini tidak punya nama domain yang bisa diperiksa."); return; }
    setBusy(d);
    try {
      const [http, https] = await Promise.all([api.checkUrl(`http://${d}/`), api.checkUrl(`https://${d}/`)]);
      let cert: Check["cert"];
      if (s.ssl_certificate) {
        try { cert = await api.certInfo(serverId, s.ssl_certificate); } catch (e) { cert = { error: toAppErr(e).message }; }
      }
      setChecks((c) => ({ ...c, [d]: { http, https, cert } }));
    } catch (e) { fail(e, "Pemeriksaan gagal"); } finally { setBusy(null); }
  };

  const test = async () => {
    setTesting(true);
    try { setNginx(await api.nginxTest(serverId)); } catch (e) { fail(e, "nginx -t gagal dijalankan"); } finally { setTesting(false); }
  };

  return (
    <div>
      <PageHeader title="Websites" description="Domain dan virtual host dibaca dari konfigurasi Nginx (hanya-baca). Aplikasi tidak mengubah konfigurasi website atau SSL."
        actions={<>
          <Button onClick={() => void reload()}><RefreshCw className="size-4" aria-hidden />Muat ulang</Button>
          <Button onClick={test} loading={testing}><Stethoscope className="size-4" aria-hidden />Validasi nginx -t</Button>
          <Button variant="primary" onClick={() => setReloadAsk(true)}>Reload Nginx</Button>
        </>} />
      {nginx && (
        <div className={`mb-4 rounded-md border p-3 ${nginx.ok ? "border-line" : "border-dashed border-fg"}`} role={nginx.ok ? "status" : "alert"}>
          <Badge tone={nginx.ok ? "ok" : "error"}>{nginx.ok ? "Konfigurasi valid" : "Konfigurasi tidak valid"}</Badge>
          <pre className="mt-2 whitespace-pre-wrap break-words font-mono text-xs text-fg2">{nginx.output}</pre>
        </div>
      )}
      <Card pad={false}>
        {loading && !data ? <Spinner label="Membaca konfigurasi Nginx…" /> : error ? <ErrorState message={error} onRetry={() => void reload()} /> :
          !data || data.length === 0 ? <EmptyState icon={<Globe className="size-8" />} title="Tidak ada virtual host" description="Nginx tidak memiliki blok server, atau Nginx belum terpasang." /> : (
          <Table>
            <thead><tr><Th>Domain</Th><Th>Root</Th><Th>Listen</Th><Th>SSL</Th><Th>HTTP</Th><Th>HTTPS</Th><Th>Sertifikat</Th><Th className="text-right">Aksi</Th></tr></thead>
            <tbody>
              {data.map((s, i) => {
                const d = primaryDomain(s);
                const c = d ? checks[d] : undefined;
                return (
                  <Tr key={i}>
                    <Td className="font-medium">{s.server_names.join(", ") || "(default)"}</Td>
                    <Td className="max-w-56 truncate font-mono text-[13px]" title={s.root ?? ""}>{s.root ?? "—"}</Td>
                    <Td className="font-mono text-[13px]">{s.listen.join("; ") || "—"}</Td>
                    <Td>{s.has_ssl ? <Badge tone="ok"><ShieldCheck className="hidden" />SSL</Badge> : <Badge tone="neutral"><ShieldQuestion className="hidden" />Tanpa SSL</Badge>}</Td>
                    <Td><UrlBadge c={c?.http} /></Td>
                    <Td><UrlBadge c={c?.https} /></Td>
                    <Td className="text-[13px]">
                      {!c?.cert ? <span className="text-fg3">—</span> : "error" in c.cert ? <Badge tone="error">Tidak terbaca</Badge> :
                        <Badge tone={c.cert.days_left != null && c.cert.days_left < 14 ? "warn" : "ok"}>{c.cert.days_left != null ? `${c.cert.days_left} hari lagi` : c.cert.not_after}</Badge>}
                    </Td>
                    <Td className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button size="sm" loading={busy === d} disabled={!d} onClick={() => void check(s)}>Periksa</Button>
                        <Button size="sm" variant="ghost" onClick={() => setView(s)}><FileCode className="size-3.5" aria-hidden />Konfigurasi</Button>
                      </div>
                    </Td>
                  </Tr>
                );
              })}
            </tbody>
          </Table>
        )}
      </Card>
      <p className="mt-2 text-xs text-fg2">Pemeriksaan HTTP/HTTPS dilakukan dari komputer Anda ke domain tersebut, bukan dari dalam server.</p>
      {view && (
        <Modal title={`Virtual host: ${view.server_names.join(", ")}`} onClose={() => setView(null)} wide footer={<Button onClick={() => setView(null)}>Tutup</Button>}>
          <p className="mb-2 text-xs text-fg2">{view.source_file ?? "File tidak diketahui"} · mode baca</p>
          <pre className="max-h-[55vh] overflow-auto rounded-md border border-line bg-sidebar p-3 font-mono text-xs">{view.config}</pre>
        </Modal>
      )}
      {reloadAsk && (
        <ConfirmDialog title="Reload Nginx?" confirmLabel="Validasi lalu reload" onCancel={() => setReloadAsk(false)}
          message="Aplikasi akan menjalankan nginx -t lebih dulu. Reload hanya dilakukan jika konfigurasi valid. Koneksi yang sedang berjalan tidak terputus."
          onConfirm={async () => {
            try { const r = await api.nginxReload(serverId, true); notify("success", r.message); setReloadAsk(false); }
            catch (e) { fail(e, "Reload dibatalkan"); setReloadAsk(false); }
          }} />
      )}
    </div>
  );
}

export function WebsitesPage() {
  return <ServerGate>{(id) => <Content serverId={id} />}</ServerGate>;
}
