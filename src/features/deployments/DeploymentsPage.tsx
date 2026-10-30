import { useEffect, useRef, useState } from "react";
import { GitBranch, Pencil, Plus, Rocket, RotateCcw, Trash2 } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api, on } from "../../lib/api";
import type { Deployment, GitStatus, Project, ProjectInput } from "../../types";
import { useAsync } from "../../hooks/usePolling";
import { ServerGate } from "../../components/ServerGate";
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, ErrorState, Field, Input, Modal, PageHeader, Spinner, Table, Td, Textarea, Th, Tr } from "../../components/ui";
import { formatDateTime, shortHash } from "../../lib/format";

const MIGRATION = /\b(migrate|migration|migrations|db:migrate|db:push|schema:update|flyway)\b/i;

function ProjectForm({ serverId, initial, onClose, onSaved }: { serverId: number; initial: Project | null; onClose: () => void; onSaved: (p: Project) => void }) {
  const { fail } = useApp();
  const [f, setF] = useState({
    name: initial?.name ?? "", path: initial?.path ?? "/var/www/", repo_url: initial?.repo_url ?? "", branch: initial?.branch ?? "main",
    steps: (initial?.build_steps ?? []).join("\n"), restart_service: initial?.restart_service ?? "", verify_url: initial?.verify_url ?? "",
  });
  const [busy, setBusy] = useState(false);
  const set = (k: keyof typeof f, v: string) => setF((p) => ({ ...p, [k]: v }));
  const save = async () => {
    setBusy(true);
    const input: ProjectInput = {
      id: initial?.id, server_id: serverId, name: f.name, path: f.path, repo_url: f.repo_url || null, branch: f.branch,
      build_steps: f.steps.split("\n").map((s) => s.trim()).filter(Boolean), restart_service: f.restart_service || null, verify_url: f.verify_url || null,
    };
    try { onSaved(await api.deploySave(input)); } catch (e) { fail(e, "Gagal menyimpan proyek"); } finally { setBusy(false); }
  };
  return (
    <Modal title={initial ? "Ubah proyek" : "Proyek deployment baru"} wide onClose={onClose}
      footer={<><Button onClick={onClose}>Batal</Button><Button variant="primary" loading={busy} onClick={() => void save()}>Simpan</Button></>}>
      <div className="grid grid-cols-2 gap-4">
        <Field label="Nama proyek">{(id) => <Input id={id} value={f.name} onChange={(e) => set("name", e.target.value)} />}</Field>
        <Field label="Branch">{(id) => <Input id={id} value={f.branch} onChange={(e) => set("branch", e.target.value)} />}</Field>
        <div className="col-span-2"><Field label="Path repository di server" hint="Repository harus sudah di-clone di server.">{(id) => <Input id={id} className="font-mono text-[13px]" value={f.path} onChange={(e) => set("path", e.target.value)} />}</Field></div>
        <div className="col-span-2"><Field label="URL repository (informasi)">{(id) => <Input id={id} value={f.repo_url} onChange={(e) => set("repo_url", e.target.value)} placeholder="git@github.com:org/repo.git" />}</Field></div>
        <div className="col-span-2"><Field label="Langkah build (satu perintah per baris)" hint="Dijalankan berurutan di folder proyek. Perintah berbahaya (rm -rf, git reset --hard, dll.) ditolak. Langkah migrasi database butuh konfirmasi saat deploy.">
          {(id) => <Textarea id={id} rows={4} className="font-mono text-[13px]" value={f.steps} onChange={(e) => set("steps", e.target.value)} placeholder={"npm ci\nnpm run build"} />}</Field></div>
        <Field label="Service yang di-restart (opsional)">{(id) => <Input id={id} value={f.restart_service} onChange={(e) => set("restart_service", e.target.value)} placeholder="myapp.service" />}</Field>
        <Field label="URL verifikasi setelah deploy (opsional)">{(id) => <Input id={id} value={f.verify_url} onChange={(e) => set("verify_url", e.target.value)} placeholder="https://example.com/health" />}</Field>
      </div>
    </Modal>
  );
}

function LogView({ text }: { text: string }) {
  const ref = useRef<HTMLPreElement>(null);
  useEffect(() => { ref.current?.scrollTo({ top: ref.current.scrollHeight }); }, [text]);
  return <pre ref={ref} className="max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md border border-line bg-sidebar p-3 font-mono text-xs">{text || "(belum ada log)"}</pre>;
}

function Content({ serverId }: { serverId: number }) {
  const { notify, fail } = useApp();
  const projects = useAsync<Project[]>(() => api.deployProjects(serverId), [serverId]);
  const [selId, setSelId] = useState<number | null>(null);
  const sel = projects.data?.find((p) => p.id === selId) ?? projects.data?.[0] ?? null;
  const hist = useAsync<Deployment[]>(() => (sel ? api.deployHistory(sel.id) : Promise.resolve([])), [sel?.id]);
  const [git, setGit] = useState<{ s?: GitStatus; err?: string; loading?: boolean }>({});
  const [form, setForm] = useState<Project | "new" | null>(null);
  const [delAsk, setDelAsk] = useState<Project | null>(null);
  const [deployAsk, setDeployAsk] = useState(false);
  const [rollbackAsk, setRollbackAsk] = useState<Deployment | null>(null);
  const [migOk, setMigOk] = useState(false);
  const [live, setLive] = useState<string[] | null>(null);
  const [openLog, setOpenLog] = useState<Deployment | null>(null);

  const hasMig = !!sel?.build_steps.some((s) => MIGRATION.test(s));

  const loadGit = async (fetch: boolean) => {
    if (!sel) return;
    setGit({ loading: true });
    try { setGit({ s: await api.deployGit(sel.id, fetch) }); } catch (e) { setGit({ err: (e as { message: string }).message }); }
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { setGit({}); if (sel) void loadGit(false); }, [sel?.id]);
  useEffect(() => {
    const u = on<{ id: number; line: string }>("deploy-log", (p) => setLive((l) => (l ? [...l, p.line] : l)));
    return () => { void u.then((f) => f()); };
  }, []);

  const run = async (kind: "deploy" | "rollback", dep?: Deployment) => {
    if (!sel) return;
    setLive([]); setDeployAsk(false); setRollbackAsk(null);
    try {
      const d = kind === "deploy" ? await api.deployRun(sel.id, migOk) : await api.deployRollback(dep!.id, migOk);
      notify(d.status === "success" ? "success" : "error", d.status === "success" ? `${kind === "deploy" ? "Deployment" : "Rollback"} berhasil dan terverifikasi.` : "Proses gagal. Lihat log untuk detail.");
      setOpenLog(d);
    } catch (e) { fail(e, "Proses dibatalkan"); }
    setLive(null); setMigOk(false); void hist.reload(); void loadGit(false);
  };

  return (
    <div>
      <PageHeader title="Deployments" description="Deploy berbasis Git: fast-forward saja, dibatalkan bila ada perubahan lokal, tanpa git reset --hard. Rollback memindahkan ke commit sebelum deployment."
        actions={<Button variant="primary" onClick={() => setForm("new")}><Plus className="size-4" aria-hidden />Proyek baru</Button>} />
      {projects.loading && !projects.data ? <Spinner /> : projects.error ? <ErrorState message={projects.error} onRetry={() => void projects.reload()} /> :
        !projects.data || projects.data.length === 0 ? (
        <Card><EmptyState icon={<Rocket className="size-8" />} title="Belum ada proyek deployment" description="Daftarkan repository yang sudah ada di server: path, branch, langkah build, dan service yang perlu di-restart." action={<Button variant="primary" onClick={() => setForm("new")}>Tambah proyek</Button>} /></Card>
      ) : (
        <div className="grid grid-cols-[240px_1fr] gap-4">
          <Card title="Proyek" pad={false}>
            <ul>{projects.data.map((p) => (
              <li key={p.id}><button type="button" onClick={() => setSelId(p.id)} aria-current={sel?.id === p.id}
                className={`block w-full border-b border-line px-4 py-3 text-left hover:bg-hover ${sel?.id === p.id ? "bg-hover font-semibold" : ""}`}>
                {p.name}<span className="block truncate font-mono text-xs font-normal text-fg2">{p.path}</span></button></li>))}</ul>
          </Card>
          {sel && (
            <div className="space-y-4">
              <Card title={sel.name} actions={<>
                <Button size="sm" onClick={() => setForm(sel)}><Pencil className="size-3.5" aria-hidden />Ubah</Button>
                <Button size="sm" onClick={() => setDelAsk(sel)}><Trash2 className="size-3.5" aria-hidden />Hapus</Button></>}>
                <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-[13px]">
                  <div><dt className="text-fg2">Path</dt><dd className="font-mono">{sel.path}</dd></div>
                  <div><dt className="text-fg2">Branch</dt><dd className="font-mono">{sel.branch}</dd></div>
                  <div><dt className="text-fg2">Restart service</dt><dd className="font-mono">{sel.restart_service ?? "—"}</dd></div>
                  <div><dt className="text-fg2">Verifikasi</dt><dd className="break-all font-mono">{sel.verify_url ?? "—"}</dd></div>
                  <div className="col-span-2"><dt className="text-fg2">Langkah build</dt><dd className="font-mono">{sel.build_steps.length ? sel.build_steps.map((s, i) => <div key={i}>{i + 1}. {s}</div>) : "—"}</dd></div>
                </dl>
              </Card>
              <Card title="Status Git" actions={<><Button size="sm" onClick={() => void loadGit(true)} loading={git.loading}><GitBranch className="size-3.5" aria-hidden />Fetch & periksa</Button>
                <Button size="sm" variant="primary" onClick={() => setDeployAsk(true)} disabled={live !== null}><Rocket className="size-3.5" aria-hidden />Deploy</Button></>}>
                {git.loading && !git.s ? <Spinner /> : git.err ? <ErrorState message={git.err} onRetry={() => void loadGit(false)} title="Tidak dapat membaca Git" /> : git.s ? (
                  <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-[13px]">
                    <div><dt className="text-fg2">Branch aktif</dt><dd className="font-mono">{git.s.branch}</dd></div>
                    <div><dt className="text-fg2">Perubahan lokal</dt><dd><Badge tone={git.s.dirty ? "warn" : "ok"}>{git.s.dirty ? `${git.s.changed_files.length} file berubah` : "Bersih"}</Badge></dd></div>
                    <div className="col-span-2"><dt className="text-fg2">Commit</dt><dd><span className="font-mono">{shortHash(git.s.head)}</span> — {git.s.head_subject} <span className="text-fg2">({git.s.head_author}, {git.s.head_date})</span></dd></div>
                    <div><dt className="text-fg2">Terhadap origin/{sel.branch}</dt><dd>{git.s.ahead_behind ? `${git.s.ahead_behind[1]} commit tertinggal, ${git.s.ahead_behind[0]} di depan` : "Belum diketahui (jalankan Fetch)"}</dd></div>
                    {git.s.dirty && <div className="col-span-2"><dt className="text-fg2">File berubah</dt><dd className="font-mono text-xs">{git.s.changed_files.slice(0, 10).map((f, i) => <div key={i}>{f}</div>)}</dd></div>}
                  </dl>) : null}
              </Card>
              {live !== null && <Card title="Log proses berjalan"><LogView text={live.join("\n")} /></Card>}
              <Card title="Riwayat deployment" pad={false}>
                {!hist.data || hist.data.length === 0 ? <EmptyState title="Belum ada deployment" /> : (
                  <Table>
                    <thead><tr><Th>Waktu</Th><Th>Jenis</Th><Th>Status</Th><Th>Dari → Ke</Th><Th className="text-right">Aksi</Th></tr></thead>
                    <tbody>{hist.data.map((d) => (
                      <Tr key={d.id}>
                        <Td className="whitespace-nowrap text-fg2">{formatDateTime(d.started_at)}</Td>
                        <Td>{d.kind === "rollback" ? "Rollback" : "Deploy"}</Td>
                        <Td><Badge tone={d.status === "success" ? "ok" : d.status === "running" ? "active" : "error"}>{d.status === "success" ? "Berhasil" : d.status === "running" ? "Berjalan" : "Gagal"}</Badge></Td>
                        <Td className="font-mono text-[13px]">{shortHash(d.from_commit)} → {shortHash(d.to_commit)}</Td>
                        <Td className="text-right"><div className="flex justify-end gap-1">
                          <Button size="sm" variant="ghost" onClick={() => setOpenLog(d)}>Log</Button>
                          {d.kind === "deploy" && d.from_commit && <Button size="sm" onClick={() => setRollbackAsk(d)}><RotateCcw className="size-3.5" aria-hidden />Rollback</Button>}
                        </div></Td>
                      </Tr>))}</tbody>
                  </Table>)}
              </Card>
            </div>
          )}
        </div>
      )}
      {form && <ProjectForm serverId={serverId} initial={form === "new" ? null : form} onClose={() => setForm(null)} onSaved={(p) => { setForm(null); setSelId(p.id); void projects.reload(); }} />}
      {delAsk && <ConfirmDialog title="Hapus proyek?" destructive confirmLabel="Hapus" onCancel={() => setDelAsk(null)} message={<>Proyek <b>{delAsk.name}</b> dan riwayat deployment-nya dihapus dari aplikasi. Kode di server tidak disentuh.</>}
        onConfirm={async () => { try { await api.deployDelete(delAsk.id); setDelAsk(null); setSelId(null); void projects.reload(); } catch (e) { fail(e); setDelAsk(null); } }} />}
      {deployAsk && sel && (
        <ConfirmDialog title={`Deploy ${sel.name}?`} confirmLabel="Mulai deploy" onCancel={() => { setDeployAsk(false); setMigOk(false); }} onConfirm={() => run("deploy")}
          message={<>Urutan: cek perubahan lokal → fetch → checkout <b className="font-mono">{sel.branch}</b> → merge --ff-only → {sel.build_steps.length} langkah build{sel.restart_service ? ` → restart ${sel.restart_service}` : ""}{sel.verify_url ? " → verifikasi URL" : ""}. Jika ada perubahan lokal, deploy dibatalkan.</>}>
          {hasMig && <div className="rounded-md border border-dashed border-fg p-3"><Checkbox checked={migOk} onChange={setMigOk} label="Saya memahami langkah build ini menjalankan migrasi database dan mengizinkannya." /></div>}
        </ConfirmDialog>)}
      {rollbackAsk && sel && (
        <ConfirmDialog title="Rollback ke versi sebelumnya?" destructive confirmLabel="Rollback" onCancel={() => { setRollbackAsk(null); setMigOk(false); }} onConfirm={() => run("rollback", rollbackAsk)}
          message={<>Repository akan di-checkout (detached HEAD) ke commit <span className="font-mono">{shortHash(rollbackAsk.from_commit)}</span>, lalu build dan restart dijalankan ulang. Rollback kode <b>tidak</b> membatalkan migrasi database. Deploy berikutnya akan kembali ke branch {sel.branch}.</>}>
          {hasMig && <div className="rounded-md border border-dashed border-fg p-3"><Checkbox checked={migOk} onChange={setMigOk} label="Saya mengizinkan langkah migrasi dijalankan ulang." /></div>}
        </ConfirmDialog>)}
      {openLog && <Modal title={`Log #${openLog.id} — ${openLog.status}`} wide onClose={() => setOpenLog(null)} footer={<Button onClick={() => setOpenLog(null)}>Tutup</Button>}><LogView text={openLog.log} /></Modal>}
    </div>
  );
}

export function DeploymentsPage() {
  return <ServerGate>{(id) => <Content serverId={id} />}</ServerGate>;
}
