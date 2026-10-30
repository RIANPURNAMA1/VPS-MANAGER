import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowUp, Download, File as FileIcon, FilePenLine, Folder, FolderPlus, Link2, Pencil, RefreshCw, Trash2, Upload } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api, on, toAppErr } from "../../lib/api";
import type { FileEntry, Listing, TransferProgress } from "../../types";
import { ServerGate } from "../../components/ServerGate";
import { Button, Card, ConfirmDialog, EmptyState, ErrorState, Field, IconButton, Input, Modal, PageHeader, Progress, Spinner, Table, Td, Textarea, Th, Tr } from "../../components/ui";
import { formatBytes, formatDateTime } from "../../lib/format";

const fileType = (e: FileEntry) => (e.is_dir ? "Folder" : e.is_symlink ? "Tautan" : (e.name.includes(".") ? e.name.split(".").pop()!.toUpperCase() : "File"));

function Content({ serverId }: { serverId: number }) {
  const { notify, fail, active, settings } = useApp();
  const start = active?.username === "root" ? "/root" : `/home/${active?.username ?? ""}`;
  const [path, setPath] = useState(start);
  const [pathInput, setPathInput] = useState(start);
  const [list, setList] = useState<Listing | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [transfers, setTransfers] = useState<Record<string, TransferProgress>>({});
  const [del, setDel] = useState<FileEntry | null>(null);
  const [rename, setRename] = useState<FileEntry | null>(null);
  const [newName, setNewName] = useState("");
  const [mkdir, setMkdir] = useState(false);
  const [editor, setEditor] = useState<{ path: string; text: string; orig: string } | null>(null);
  const [saveAsk, setSaveAsk] = useState(false);
  const [overwrite, setOverwrite] = useState<{ local: string; name: string } | null>(null);
  const alive = useRef(true);

  const load = useCallback(async (p: string) => {
    setLoading(true);
    try {
      const l = await api.filesList(serverId, p);
      if (!alive.current) return;
      setList(l); setPath(l.path); setPathInput(l.path); setErr(null);
    } catch (e) { if (alive.current) setErr(toAppErr(e).message); } finally { if (alive.current) setLoading(false); }
  }, [serverId]);

  useEffect(() => { alive.current = true; void load(start); return () => { alive.current = false; }; }, [load, start]);
  useEffect(() => {
    const u = on<TransferProgress>("transfer-progress", (p) => setTransfers((t) => ({ ...t, [p.id]: p })));
    return () => { void u.then((f) => f()); };
  }, []);

  const startUpload = async (local: string, ow: boolean) => {
    const tid = `up-${Date.now()}`;
    const name = local.split(/[\\/]/).pop() ?? local;
    setTransfers((t) => ({ ...t, [tid]: { id: tid, name, direction: "upload", done: 0, total: 1, finished: false } }));
    try {
      await api.filesUpload(serverId, local, path, ow, tid);
      notify("success", `${name} terunggah.`); void load(path);
    } catch (e) {
      setTransfers((t) => { const n = { ...t }; delete n[tid]; return n; });
      const er = toAppErr(e);
      if (er.code === "exists") setOverwrite({ local, name }); else fail(er, "Unggah gagal");
    }
  };
  const upload = async () => {
    const r = await open({ multiple: false, directory: false, title: "Pilih file untuk diunggah" });
    if (typeof r === "string") await startUpload(r, false);
  };
  const download = async (e: FileEntry) => {
    const tid = `dl-${Date.now()}`;
    setTransfers((t) => ({ ...t, [tid]: { id: tid, name: e.name, direction: "download", done: 0, total: e.size || 1, finished: false } }));
    try { const p = await api.filesDownload(serverId, e.path, tid); notify("success", `Tersimpan di ${p}`); }
    catch (er) { setTransfers((t) => { const n = { ...t }; delete n[tid]; return n; }); fail(er, "Unduh gagal"); }
  };
  const edit = async (e: FileEntry) => {
    try { const text = await api.filesReadText(serverId, e.path); setEditor({ path: e.path, text, orig: text }); }
    catch (er) { fail(er, "Tidak dapat membuka file"); }
  };

  const active_tr = Object.values(transfers);
  return (
    <div>
      <PageHeader title="File Manager" description="Jelajahi server lewat SFTP. Penghapusan, penimpaan, dan penyimpanan perubahan selalu meminta konfirmasi."
        actions={<>
          <Button onClick={() => { setNewName(""); setMkdir(true); }}><FolderPlus className="size-4" aria-hidden />Folder baru</Button>
          <Button variant="primary" onClick={() => void upload()}><Upload className="size-4" aria-hidden />Unggah</Button>
        </>} />
      <form className="mb-3 flex gap-2" onSubmit={(e) => { e.preventDefault(); void load(pathInput); }}>
        <IconButton label="Ke folder induk" disabled={!list?.parent} onClick={() => list?.parent && void load(list.parent)}><ArrowUp className="size-4" /></IconButton>
        <IconButton label="Muat ulang folder" onClick={() => void load(path)}><RefreshCw className="size-4" /></IconButton>
        <Input aria-label="Path lengkap" className="font-mono text-[13px]" value={pathInput} onChange={(e) => setPathInput(e.target.value)} />
        <Button type="submit">Buka</Button>
      </form>
      {active_tr.length > 0 && (
        <Card className="mb-3" title="Transfer">
          <ul className="space-y-2">
            {active_tr.map((t) => (
              <li key={t.id} className="flex items-center gap-3">
                <span className="w-56 truncate text-[13px]">{t.direction === "upload" ? "↑" : "↓"} {t.name}</span>
                <div className="flex-1"><Progress value={t.finished ? 100 : (t.done / Math.max(1, t.total)) * 100} label={`Transfer ${t.name}`} /></div>
                <span className="w-32 text-right text-xs text-fg2">{t.finished ? "Selesai" : `${formatBytes(t.done)} / ${formatBytes(t.total)}`}</span>
                {t.finished && <button type="button" className="text-xs text-fg2 underline" onClick={() => setTransfers((x) => { const n = { ...x }; delete n[t.id]; return n; })}>Tutup</button>}
              </li>))}
          </ul>
          {settings && <p className="mt-2 text-xs text-fg2">Batas ukuran transfer: {settings.max_transfer_mb} MB (ubah di Pengaturan).</p>}
        </Card>
      )}
      <Card pad={false}>
        {loading && !list ? <Spinner /> : err ? <ErrorState message={err} onRetry={() => void load(path)} title="Tidak dapat membuka folder" /> :
          !list || list.entries.length === 0 ? <EmptyState icon={<Folder className="size-8" />} title="Folder kosong" description={list?.path} /> : (
          <Table>
            <thead><tr><Th>Nama</Th><Th>Ukuran</Th><Th>Tipe</Th><Th>Diubah</Th><Th>Izin</Th><Th className="text-right">Aksi</Th></tr></thead>
            <tbody>
              {list.entries.map((e) => (
                <Tr key={e.path}>
                  <Td>
                    {e.is_dir ? (
                      <button type="button" className="flex items-center gap-2 font-medium hover:underline" onClick={() => void load(e.path)}><Folder className="size-4" aria-hidden />{e.name}</button>
                    ) : <span className="flex items-center gap-2">{e.is_symlink ? <Link2 className="size-4" aria-hidden /> : <FileIcon className="size-4" aria-hidden />}{e.name}</span>}
                  </Td>
                  <Td className="tabular-nums">{e.is_dir ? "—" : formatBytes(e.size)}</Td>
                  <Td className="text-fg2">{fileType(e)}</Td>
                  <Td className="text-fg2">{formatDateTime(e.mtime)}</Td>
                  <Td className="font-mono text-xs text-fg2" title={e.perm_octal}>{e.perm}</Td>
                  <Td className="text-right"><div className="flex justify-end">
                    {!e.is_dir && <IconButton label={`Unduh ${e.name}`} onClick={() => void download(e)}><Download className="size-4" /></IconButton>}
                    {!e.is_dir && <IconButton label={`Edit ${e.name}`} onClick={() => void edit(e)}><FilePenLine className="size-4" /></IconButton>}
                    <IconButton label={`Ganti nama ${e.name}`} onClick={() => { setRename(e); setNewName(e.name); }}><Pencil className="size-4" /></IconButton>
                    <IconButton label={`Hapus ${e.name}`} onClick={() => setDel(e)}><Trash2 className="size-4" /></IconButton>
                  </div></Td>
                </Tr>))}
            </tbody>
          </Table>
        )}
      </Card>

      {del && (
        <ConfirmDialog title={del.is_dir ? "Hapus folder?" : "Hapus file?"} destructive confirmLabel="Hapus" onCancel={() => setDel(null)}
          message={<>Path lengkap yang akan dihapus permanen:<pre className="mt-2 break-all rounded-md border border-line bg-sidebar p-2 font-mono text-[13px]">{del.path}</pre>{del.is_dir && <p className="mt-2 text-fg2">Hanya folder kosong yang dapat dihapus.</p>}</>}
          onConfirm={async () => { try { await api.filesDelete(serverId, del.path, true); notify("success", "Dihapus."); setDel(null); void load(path); } catch (e) { fail(e, "Gagal menghapus"); setDel(null); } }} />
      )}
      {rename && (
        <Modal title="Ganti nama" onClose={() => setRename(null)} footer={<><Button onClick={() => setRename(null)}>Batal</Button>
          <Button variant="primary" onClick={async () => { try { await api.filesRename(serverId, rename.path, newName.trim()); setRename(null); void load(path); } catch (e) { fail(e, "Gagal mengganti nama"); } }}>Simpan</Button></>}>
          <Field label="Nama baru" hint={`Lokasi: ${rename.path}`}>{(id) => <Input id={id} value={newName} onChange={(e) => setNewName(e.target.value)} />}</Field>
        </Modal>
      )}
      {mkdir && (
        <Modal title="Folder baru" onClose={() => setMkdir(false)} footer={<><Button onClick={() => setMkdir(false)}>Batal</Button>
          <Button variant="primary" onClick={async () => { try { await api.filesMkdir(serverId, path, newName.trim()); setMkdir(false); void load(path); } catch (e) { fail(e, "Gagal membuat folder"); } }}>Buat</Button></>}>
          <Field label="Nama folder" hint={`Dibuat di ${path}`}>{(id) => <Input id={id} value={newName} onChange={(e) => setNewName(e.target.value)} />}</Field>
        </Modal>
      )}
      {overwrite && (
        <ConfirmDialog title="Timpa file di server?" destructive confirmLabel="Timpa" onCancel={() => setOverwrite(null)}
          message={<><b>{overwrite.name}</b> sudah ada di {path}. File di server akan diganti dengan file lokal.</>}
          onConfirm={async () => { const o = overwrite; setOverwrite(null); await startUpload(o.local, true); }} />
      )}
      {editor && (
        <Modal title={`Edit: ${editor.path}`} wide onClose={() => setEditor(null)}
          footer={<><span className="mr-auto self-center text-xs text-fg2">{editor.text === editor.orig ? "Tidak ada perubahan" : "Ada perubahan belum disimpan"}</span>
            <Button onClick={() => setEditor(null)}>Tutup</Button>
            <Button variant="primary" disabled={editor.text === editor.orig} onClick={() => setSaveAsk(true)}>Simpan…</Button></>}>
          <Textarea aria-label="Isi file" spellCheck={false} className="h-[50vh] resize-none font-mono text-[13px]" value={editor.text} onChange={(e) => setEditor({ ...editor, text: e.target.value })} />
        </Modal>
      )}
      {saveAsk && editor && (
        <ConfirmDialog title="Simpan perubahan ke server?" confirmLabel="Simpan" onCancel={() => setSaveAsk(false)}
          message={<>File <span className="font-mono">{editor.path}</span> di server akan diganti. Hak akses file dipertahankan. Menyimpan dilakukan secara atomik (file sementara lalu rename).</>}
          onConfirm={async () => { try { await api.filesWriteText(serverId, editor.path, editor.text, true); notify("success", "File disimpan."); setEditor({ ...editor, orig: editor.text }); setSaveAsk(false); } catch (e) { fail(e, "Gagal menyimpan"); setSaveAsk(false); } }} />
      )}
    </div>
  );
}

export function FilesPage() {
  return <ServerGate>{(id) => <Content serverId={id} />}</ServerGate>;
}
