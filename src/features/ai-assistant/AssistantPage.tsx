import { useEffect, useRef, useState } from "react";
import { Bot, Play, Send, ShieldAlert } from "lucide-react";
import { useApp } from "../../app/AppContext";
import { api } from "../../lib/api";
import type { AiStatus, ChatMessage, CommandResult, Overview, Suggestion } from "../../types";
import { Badge, Button, Card, ConfirmDialog, EmptyState, PageHeader, Select, Spinner, Textarea } from "../../components/ui";

type Msg = ChatMessage & { suggestions?: Suggestion[] };

const riskLabel = { safe: "Baca-saja", caution: "Perlu konfirmasi", blocked: "Diblokir" } as const;
const riskTone = { safe: "ok", caution: "warn", blocked: "error" } as const;

export function AssistantPage() {
  const { active, isConnected, fail, settings } = useApp();
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [units, setUnits] = useState<string[]>([]);
  const [unit, setUnit] = useState("");
  const [results, setResults] = useState<Record<string, CommandResult | { error: string }>>({});
  const [ask, setAsk] = useState<Suggestion | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => { api.aiStatus().then(setStatus).catch(() => setStatus({ available: false, models: [], error: "Gagal memeriksa Ollama." })); }, []);
  useEffect(() => {
    if (!active || !isConnected) { setUnits([]); return; }
    api.servicesOverview(active.id).then((o: Overview) => setUnits(o.services.filter((s) => s.load === "loaded").map((s) => s.name))).catch(() => setUnits([]));
  }, [active, isConnected]);
  useEffect(() => { end.current?.scrollIntoView({ block: "end" }); }, [msgs, busy]);

  const send = async (text: string) => {
    if (!text.trim() || busy) return;
    const next: Msg[] = [...msgs, { role: "user", content: text }];
    setMsgs(next); setInput(""); setBusy(true);
    try {
      const r = await api.aiChat(next.map(({ role, content }) => ({ role, content })));
      setMsgs([...next, { role: "assistant", content: r.content, suggestions: r.suggestions }]);
    } catch (e) { fail(e, "Asisten AI"); } finally { setBusy(false); }
  };

  const analyzeLogs = async () => {
    if (!active || !unit) return;
    try {
      const logs = await api.serviceLogs(active.id, unit, 150);
      await send(`Analisis log service ${unit} berikut. Jelaskan error utama, penyebab paling mungkin, dan langkah perbaikan bertahap.\n\n${logs.slice(-9000)}`);
    } catch (e) { fail(e, "Gagal mengambil log"); }
  };

  const run = async (s: Suggestion, confirmed: boolean) => {
    if (!active) return;
    setRunning(s.command);
    try { const r = await api.aiRun(active.id, s.command, confirmed); setResults((x) => ({ ...x, [s.command]: r })); }
    catch (e) { setResults((x) => ({ ...x, [s.command]: { error: (e as { message: string }).message } })); }
    finally { setRunning(null); }
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="AI Assistant" description="Analisis log dan saran perbaikan lewat Ollama lokal. AI tidak pernah menjalankan perintah sendiri; semua perintah diklasifikasi risikonya dan butuh persetujuan Anda." />
      <div className="mb-3 flex flex-wrap items-center gap-3">
        {!status ? <Spinner label="Memeriksa Ollama…" /> : status.available ? <Badge tone="ok">Ollama aktif</Badge> : <Badge tone="error">Ollama tidak tersedia</Badge>}
        {status?.available && <span className="text-[13px] text-fg2">Model: {settings?.ollama_model} {status.models.length > 0 && !status.models.some((m) => m.startsWith(settings?.ollama_model ?? "§")) && "(belum di-pull)"}</span>}
        <div className="ml-auto flex items-center gap-2">
          <Select aria-label="Service untuk dianalisis" className="h-8 w-56" value={unit} onChange={(e) => setUnit(e.target.value)} disabled={!isConnected}>
            <option value="">Pilih service untuk dianalisis…</option>{units.map((u) => <option key={u} value={u}>{u}</option>)}
          </Select>
          <Button onClick={() => void analyzeLogs()} disabled={!unit || busy || !status?.available}>Analisis log</Button>
        </div>
      </div>
      {status && !status.available && (
        <p className="mb-3 rounded-md border border-dashed border-fg p-3 text-[13px]">Fitur lain tetap berfungsi tanpa AI. Untuk mengaktifkan: pasang Ollama, jalankan <code className="font-mono">ollama pull {settings?.ollama_model ?? "llama3.2"}</code>, lalu muat ulang halaman. {status.error}</p>
      )}
      <Card className="flex min-h-0 flex-1 flex-col" pad={false}>
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
          {msgs.length === 0 && <EmptyState icon={<Bot className="size-8" />} title="Tanyakan apa saja tentang server Anda" description="Contoh: “Kenapa Nginx gagal start?” atau pilih service lalu klik Analisis log. Log dan teks disamarkan (password, token, key) sebelum dikirim ke model." />}
          {msgs.map((m, i) => (
            <div key={i} className={m.role === "user" ? "ml-16" : "mr-16"}>
              <div className="mb-1 text-xs text-fg2">{m.role === "user" ? "Anda" : "Asisten"}</div>
              <div className={`whitespace-pre-wrap break-words rounded-md border p-3 ${m.role === "user" ? "border-line bg-sidebar" : "border-line bg-card"}`}>{m.content.length > 1500 && m.role === "user" ? `${m.content.slice(0, 600)}\n… (${m.content.length} karakter)` : m.content}</div>
              {m.suggestions && m.suggestions.length > 0 && (
                <div className="mt-2 space-y-2">
                  <p className="text-xs text-fg2">Perintah yang diusulkan (belum dijalankan):</p>
                  {m.suggestions.map((s) => {
                    const r = results[s.command];
                    return (
                      <div key={s.command} className="rounded-md border border-line p-3">
                        <div className="flex items-center gap-2">
                          <code className="min-w-0 flex-1 break-all font-mono text-[13px]">{s.command}</code>
                          <Badge tone={riskTone[s.assessment.risk]}>{riskLabel[s.assessment.risk]}</Badge>
                          <Button size="sm" disabled={s.assessment.risk === "blocked" || !isConnected || running === s.command} loading={running === s.command}
                            onClick={() => (s.assessment.risk === "safe" ? void run(s, false) : setAsk(s))}>
                            {s.assessment.risk === "blocked" ? <ShieldAlert className="size-3.5" aria-hidden /> : <Play className="size-3.5" aria-hidden />}Jalankan
                          </Button>
                        </div>
                        <p className="mt-1 text-xs text-fg2">{s.assessment.reason}</p>
                        {r && ("error" in r ? <p className="mt-2 text-[13px]">Ditolak/gagal: {r.error}</p> :
                          <pre className="mt-2 max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-md border border-line bg-sidebar p-2 font-mono text-xs">{`exit ${r.exit}\n${r.output || "(tanpa output)"}`}</pre>)}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
          {busy && <Spinner label="Model sedang menjawab…" />}
          <div ref={end} />
        </div>
        <form className="flex gap-2 border-t border-line p-3" onSubmit={(e) => { e.preventDefault(); void send(input); }}>
          <Textarea aria-label="Pesan" rows={2} className="flex-1 resize-none" placeholder="Tulis pertanyaan…" value={input} onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void send(input); } }} />
          <Button type="submit" variant="primary" className="self-end" disabled={!input.trim() || busy || !status?.available}><Send className="size-4" aria-hidden />Kirim</Button>
        </form>
      </Card>
      {ask && (
        <ConfirmDialog title="Jalankan perintah berisiko?" destructive confirmLabel="Jalankan" onCancel={() => setAsk(null)}
          message={<>Perintah ini dapat mengubah sistem. Alasan: {ask.assessment.reason}.<pre className="mt-3 whitespace-pre-wrap break-all rounded-md border border-line bg-sidebar p-3 font-mono text-[13px]">{ask.command}</pre>Dijalankan di <b>{active?.name}</b>. Output akan disamarkan dan aktivitas dicatat.</>}
          onConfirm={async () => { const s = ask; setAsk(null); await run(s, true); }} />
      )}
    </div>
  );
}
