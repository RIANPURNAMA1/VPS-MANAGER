export interface Profile {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: "key" | "password";
  key_path: string | null;
  use_sudo: boolean;
  mysql_defaults_file: string | null;
  created_at: string;
  has_secret: boolean;
}

export interface ProfileInput {
  id?: number;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: "key" | "password";
  key_path: string | null;
  use_sudo: boolean;
  mysql_defaults_file: string | null;
  /** undefined = pertahankan, "" = hapus */
  secret?: string;
}

export interface TestResult { ok: boolean; latency_ms: number; remote: string }
export interface KnownHost { host: string; port: number; key_type: string; fingerprint: string; trusted_at: string }

export interface DiskInfo {
  filesystem: string; fstype: string; mount: string;
  total_bytes: number; used_bytes: number; avail_bytes: number; used_percent: number;
}
export interface Metrics {
  collected_at: string; hostname: string; os: string; kernel: string; cpu_count: number;
  cpu_percent: number | null; load: [number, number, number];
  mem_total: number; mem_used: number; mem_available: number; swap_total: number; swap_used: number;
  disks: DiskInfo[]; uptime_secs: number;
  net_rx_bps: number | null; net_tx_bps: number | null; net_rx_total: number; net_tx_total: number;
}

export interface ServiceInfo { name: string; load: string; active: string; sub: string; description: string }
export interface ProcessInfo { pid: number; user: string; cpu: number; mem: number; rss_kb: number; elapsed: string; command: string }
export interface Pm2Process { name: string; pid: number | null; status: string; cpu: number | null; memory: number | null; restarts: number | null }
export interface Detected { nginx: string | null; database: string | null; php_fpm: string[]; ssh: string | null }
export interface Overview { services: ServiceInfo[]; detected: Detected; pm2_available: boolean; pm2: Pm2Process[] }
export interface ActionResult { unit: string; action: string; exit: number; active_after: string; message: string }

export interface Site {
  server_names: string[]; root: string | null; listen: string[]; ssl_certificate: string | null;
  has_ssl: boolean; source_file: string | null; config: string;
}
export interface UrlCheck { url: string; ok: boolean; status: number | null; error: string | null; elapsed_ms: number }
export interface CertInfo { not_after: string; days_left: number | null }
export interface NginxTest { ok: boolean; output: string }

export interface FileEntry {
  name: string; path: string; is_dir: boolean; is_symlink: boolean; size: number;
  mtime: number | null; perm: string; perm_octal: string; uid: number | null;
}
export interface Listing { path: string; parent: string | null; entries: FileEntry[] }
export interface TransferProgress { id: string; name: string; direction: "upload" | "download"; done: number; total: number; finished: boolean }

export interface DbInfo { name: string; size_bytes: number | null; is_system: boolean }
export interface BackupRecord {
  id: number; server_id: number; database_name: string; remote_path: string; local_path: string | null;
  size_bytes: number | null; sha256: string | null; status: string; verified: boolean; note: string | null; created_at: string;
}
export interface BackupBatch { records: BackupRecord[]; succeeded: number; failed: number }
export interface RemoteBackup { path: string; size_bytes: number; modified: number }

export interface Project {
  id: number; server_id: number; name: string; path: string; repo_url: string | null; branch: string;
  build_steps: string[]; restart_service: string | null; verify_url: string | null;
}
export interface ProjectInput extends Omit<Project, "id"> { id?: number }
export interface GitStatus {
  branch: string; head: string; head_subject: string; head_author: string; head_date: string;
  dirty: boolean; changed_files: string[]; ahead_behind: [number, number] | null;
}
export interface Deployment {
  id: number; project_id: number; kind: string; started_at: string; finished_at: string | null;
  status: string; from_commit: string | null; to_commit: string | null; log: string;
}

export type Risk = "safe" | "caution" | "blocked";
export interface ChatMessage { role: "user" | "assistant"; content: string }
export interface Suggestion { command: string; assessment: { risk: Risk; reason: string } }
export interface ChatReply { content: string; suggestions: Suggestion[] }
export interface AiStatus { available: boolean; models: string[]; error: string | null }
export interface CommandResult { command: string; risk: Risk; exit: number; output: string }

export interface LogEntry {
  id: number; ts: string; server_id: number | null; server_name: string | null; action: string;
  status: string; exit_code: number | null; duration_ms: number | null; summary: string | null;
}

export interface AppSettings {
  theme: "light" | "dark" | "system";
  monitor_interval_secs: number; ssh_timeout_secs: number; download_dir: string; max_transfer_mb: number;
  ollama_url: string; ollama_model: string; log_retention_days: number;
}
