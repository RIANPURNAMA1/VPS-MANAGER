import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { KeyRound, Pencil, Plug, PlugZap, Plus, Server, Trash2, Zap } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api, toAppErr } from "../../lib/api";
import type { Profile, ProfileInput } from "../../types";
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, Field, IconButton, Input, Modal, PageHeader, Select, Table, Td, Th, Tr } from "../../components/ui";
import { formatDateTime } from "../../lib/format";

const blank: ProfileInput = { name: "", host: "", port: 22, username: "ubuntu", auth_type: "key", key_path: "", use_sudo: false, mysql_defaults_file: "" };

function ProfileForm({ initial, onClose, onSaved }: { initial: Profile | null; onClose: () => void; onSaved: () => void }) {
  const { withTrust, fail, notify } = useApp();
  const [f, setF] = useState<ProfileInput>(initial ? { ...initial } : blank);
  const [secret, setSecret] = useState("");
  const [clearSecret, setClearSecret] = useState(false);
  const [busy, setBusy] = useState<"" | "test" | "save">("");
  const set = <K extends keyof ProfileInput>(k: K, v: ProfileInput[K]) => setF((p) => ({ ...p, [k]: v }));

  const payload = (): ProfileInput => ({
    ...f,
    name: f.name.trim(), host: f.host.trim(), username: f.username.trim(),
    key_path: f.auth_type === "key" ? (f.key_path ?? "").trim() || null : null,
    mysql_defaults_file: (f.mysql_defaults_file ?? "").trim() || null,
    secret: clearSecret ? "" : secret ? secret : undefined,
  });

  const pick = async () => {
    const r = await open({ multiple: false, directory: false, title: "Pilih file private key" });
    if (typeof r === "string") set("key_path", r);
  };

  const test = async () => {
    setBusy("test");
    try {
      const r = await withTrust((fp) => api.testConnection(payload(), fp));
      notify("success", `Koneksi berhasil (${r.latency_ms} ms) — ${r.remote}`);
    } catch (e) {
      const err = toAppErr(e);
      if (err.code !== "cancelled") fail(err, "Uji koneksi gagal");
    } finally { setBusy(""); }
  };

  const save = async () => {
    setBusy("save");
    try { await api.saveProfile(payload()); notify("success", "Profil server disimpan."); onSaved(); }
    catch (e) { fail(e, "Gagal menyimpan"); } finally { setBusy(""); }
  };

  return (
    <Modal title={initial ? "Ubah server" : "Tambah server"} onClose={onClose} wide
      footer={<>
        <Button onClick={test} loading={busy === "test"} disabled={busy === "save"}><Zap className="size-4" aria-hidden />Uji koneksi</Button>
        <Button onClick={onClose}>Batal</Button>
        <Button variant="primary" onClick={save} loading={busy === "save"}>Simpan</Button>
      </>}>
      <div className="grid grid-cols-2 gap-4">
        <Field label="Nama server">{(id) => <Input id={id} value={f.name} onChange={(e) => set("name", e.target.value)} placeholder="Produksi" />}</Field>
        <Field label="Hostname atau IP">{(id) => <Input id={id} value={f.host} onChange={(e) => set("host", e.target.value)} placeholder="203.0.113.10" />}</Field>
        <Field label="Port SSH">{(id) => <Input id={id} type="number" min={1} max={65535} value={f.port} onChange={(e) => set("port", Number(e.target.value))} />}</Field>
        <Field label="Username">{(id) => <Input id={id} value={f.username} onChange={(e) => set("username", e.target.value)} />}</Field>
        <Field label="Autentikasi">{(id) => (
          <Select id={id} value={f.auth_type} onChange={(e) => set("auth_type", e.target.value as "key" | "password")}>
            <option value="key">SSH key (disarankan)</option><option value="password">Password</option>
          </Select>)}</Field>
        <div className="flex items-end"><Checkbox checked={f.use_sudo} onChange={(v) => set("use_sudo", v)} label="Gunakan sudo -n untuk operasi administratif" /></div>
        {f.auth_type === "key" && (
          <div className="col-span-2">
            <Field label="File private key" hint="Hanya path yang disimpan. Isi key tidak pernah disalin atau disimpan aplikasi.">{(id) => (
              <div className="flex gap-2"><Input id={id} value={f.key_path ?? ""} onChange={(e) => set("key_path", e.target.value)} placeholder="C:\Users\Anda\.ssh\id_ed25519" />
                <Button onClick={pick}><KeyRound className="size-4" aria-hidden />Pilih…</Button></div>)}</Field>
          </div>
        )}
        <div className="col-span-2">
          <Field label={f.auth_type === "key" ? "Passphrase key (opsional)" : "Password"}
            hint={initial?.has_secret ? "Sudah tersimpan di Windows Credential Manager. Kosongkan untuk mempertahankan." : "Akan disimpan di Windows Credential Manager, bukan di database atau log."}>
            {(id) => <Input id={id} type="password" autoComplete="new-password" value={secret} disabled={clearSecret} onChange={(e) => setSecret(e.target.value)} />}
          </Field>
          {initial?.has_secret && <div className="mt-2"><Checkbox checked={clearSecret} onChange={setClearSecret} label="Hapus kredensial tersimpan" /></div>}
        </div>
        <div className="col-span-2">
          <Field label="File defaults MySQL di server (opsional)" hint="Contoh: /root/.my.cnf. Dipakai agar password database tidak muncul di command line.">
            {(id) => <Input id={id} value={f.mysql_defaults_file ?? ""} onChange={(e) => set("mysql_defaults_file", e.target.value)} placeholder="/home/ubuntu/.my.cnf" />}
          </Field>
        </div>
      </div>
    </Modal>
  );
}

export function ConnectionsPage() {
  const { profiles, connected, connect, disconnect, reloadProfiles, fail, notify, active, setActiveId } = useApp();
  const [editing, setEditing] = useState<Profile | "new" | null>(null);
  const [removing, setRemoving] = useState<Profile | null>(null);
  const [checked, setChecked] = useState<Record<number, string>>({});
  const [busyId, setBusyId] = useState<number | null>(null);

  useEffect(() => {
    // periksa koneksi yang sedang terhubung untuk mengisi "pemeriksaan terakhir"
    connected.forEach((id) => {
      api.checkConnection(id).then(() => setChecked((c) => ({ ...c, [id]: new Date().toISOString() }))).catch(() => void reloadProfiles());
    });
  }, [connected, reloadProfiles]);

  const toggle = async (p: Profile) => {
    setBusyId(p.id);
    try {
      if (connected.has(p.id)) await disconnect(p.id); else await connect(p.id);
      setChecked((c) => ({ ...c, [p.id]: new Date().toISOString() }));
      setActiveId(p.id);
    } finally { setBusyId(null); }
  };

  return (
    <div>
      <PageHeader title="Server" description="Kelola profil VPS dan koneksi SSH. Kredensial disimpan di Windows Credential Manager."
        actions={<Button variant="primary" onClick={() => setEditing("new")}><Plus className="size-4" aria-hidden />Tambah server</Button>} />
      <Card pad={false}>
        {profiles.length === 0 ? (
          <EmptyState icon={<Server className="size-8" />} title="Belum ada server" description="Tambahkan VPS Ubuntu Anda: nama, alamat, port SSH, user, dan SSH key."
            action={<Button variant="primary" onClick={() => setEditing("new")}>Tambah server</Button>} />
        ) : (
          <Table>
            <thead><tr><Th>Nama</Th><Th>Host / IP</Th><Th>Port</Th><Th>Username</Th><Th>Status</Th><Th>Terakhir diperiksa</Th><Th className="text-right">Aksi</Th></tr></thead>
            <tbody>
              {profiles.map((p) => (
                <Tr key={p.id}>
                  <Td className="font-medium">{p.name}{active?.id === p.id && <span className="ml-2 text-xs text-fg3">(aktif)</span>}</Td>
                  <Td className="font-mono text-[13px]">{p.host}</Td>
                  <Td>{p.port}</Td>
                  <Td className="font-mono text-[13px]">{p.username}</Td>
                  <Td><Badge tone={connected.has(p.id) ? "ok" : "neutral"}>{connected.has(p.id) ? "Terhubung" : "Terputus"}</Badge></Td>
                  <Td className="text-fg2">{checked[p.id] ? formatDateTime(checked[p.id]) : "—"}</Td>
                  <Td className="text-right">
                    <div className="flex justify-end gap-1">
                      <Button size="sm" variant={connected.has(p.id) ? "secondary" : "primary"} loading={busyId === p.id} onClick={() => void toggle(p)}>
                        {connected.has(p.id) ? <><Plug className="size-3.5" aria-hidden />Putuskan</> : <><PlugZap className="size-3.5" aria-hidden />Hubungkan</>}
                      </Button>
                      <IconButton label={`Ubah ${p.name}`} onClick={() => setEditing(p)}><Pencil className="size-4" /></IconButton>
                      <IconButton label={`Hapus ${p.name}`} onClick={() => setRemoving(p)}><Trash2 className="size-4" /></IconButton>
                    </div>
                  </Td>
                </Tr>
              ))}
            </tbody>
          </Table>
        )}
      </Card>
      {editing && <ProfileForm initial={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); void reloadProfiles(); }} />}
      {removing && (
        <ConfirmDialog title="Hapus server?" destructive confirmLabel="Hapus" onCancel={() => setRemoving(null)}
          message={<>Profil <b>{removing.name}</b> beserta kredensial tersimpan, proyek deployment, dan riwayat backup-nya akan dihapus dari aplikasi. Server sebenarnya tidak terpengaruh.</>}
          onConfirm={async () => {
            try { await api.deleteProfile(removing.id); notify("success", "Server dihapus."); setRemoving(null); await reloadProfiles(); }
            catch (e) { fail(e); }
          }} />
      )}
    </div>
  );
}
