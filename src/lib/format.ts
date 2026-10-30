export function formatBytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !Number.isFinite(n)) return "—";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = Math.max(0, n);
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${i === 0 ? v.toFixed(0) : v.toFixed(digits)} ${units[i]}`;
}

export function formatRate(bps: number | null | undefined): string {
  return bps == null ? "—" : `${formatBytes(bps)}/dtk`;
}

export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d} hari ${h} jam`;
  if (h > 0) return `${h} jam ${m} menit`;
  return `${m} menit`;
}

export function formatPercent(n: number | null | undefined, digits = 0): string {
  return n == null || !Number.isFinite(n) ? "—" : `${n.toFixed(digits)}%`;
}

export function formatDateTime(iso: string | number | null | undefined): string {
  if (iso == null || iso === "") return "—";
  const d = typeof iso === "number" ? new Date(iso * 1000) : new Date(iso.includes("T") || iso.includes("Z") ? iso : iso.replace(" ", "T") + "Z");
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleString("id-ID", { dateStyle: "medium", timeStyle: "short" });
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "—";
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} dtk`;
  return `${Math.floor(ms / 60000)} mnt ${Math.round((ms % 60000) / 1000)} dtk`;
}

export function timeAgo(ts: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 5) return "baru saja";
  if (s < 60) return `${s} dtk lalu`;
  if (s < 3600) return `${Math.floor(s / 60)} mnt lalu`;
  return `${Math.floor(s / 3600)} jam lalu`;
}

export function shortHash(h: string | null | undefined, n = 8): string {
  return h ? h.slice(0, n) : "—";
}

/** Pola perintah yang berpotensi merusak data (untuk konfirmasi di terminal). */
const DANGEROUS: RegExp[] = [
  /\brm\s+(-\S+\s+)*(-[a-z]*[rf][a-z]*|--recursive|--force)/i,
  /\bmkfs(\.\w+)?\b/i,
  /\bdd\s+[^|;]*\bof=/i,
  /\b(shutdown|reboot|halt|poweroff)\b/i,
  /\bdrop\s+(database|table|schema)\b|\btruncate\s+table\b/i,
  /\bchmod\s+(-R\s+)?[0-7]*777\b/,
  />\s*\/dev\/(sd|nvme|vd|xvd)/,
  /\bgit\s+reset\s+--hard\b|\bgit\s+clean\s+-\S*f/i,
  /\b(userdel|deluser|chpasswd|visudo)\b/i,
  /:\(\)\s*\{/,
];

export function isDangerousCommand(line: string): boolean {
  return DANGEROUS.some((r) => r.test(line));
}
