import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type * as T from "../types";

export interface AppErr { code: string; message: string; data?: Record<string, string | number> }

export function toAppErr(e: unknown): AppErr {
  if (e && typeof e === "object" && "message" in e && "code" in e) return e as AppErr;
  if (typeof e === "string") return { code: "unknown", message: e };
  if (e instanceof Error) return { code: "unknown", message: e.message };
  return { code: "unknown", message: "Terjadi kesalahan yang tidak diketahui." };
}

async function call<R>(cmd: string, args?: Record<string, unknown>): Promise<R> {
  try {
    return await invoke<R>(cmd, args);
  } catch (e) {
    throw toAppErr(e);
  }
}

export const api = {
  // koneksi
  listProfiles: () => call<T.Profile[]>("list_profiles"),
  saveProfile: (input: T.ProfileInput) => call<T.Profile>("save_profile", { input }),
  deleteProfile: (id: number) => call<void>("delete_profile", { id }),
  testConnection: (input: T.ProfileInput, trustFingerprint?: string) =>
    call<T.TestResult>("test_connection", { input, trustFingerprint }),
  connect: (id: number, trustFingerprint?: string) => call<number>("connect_profile", { id, trustFingerprint }),
  disconnect: (id: number) => call<void>("disconnect_profile", { id }),
  connectionStates: () => call<number[]>("connection_states"),
  checkConnection: (id: number) => call<number>("check_connection", { id }),
  knownHosts: () => call<T.KnownHost[]>("list_known_hosts"),
  forgetHost: (host: string, port: number) => call<void>("forget_known_host", { host, port }),

  // monitoring & layanan
  metrics: (serverId: number) => call<T.Metrics>("get_metrics", { serverId }),
  servicesOverview: (serverId: number) => call<T.Overview>("services_overview", { serverId }),
  processes: (serverId: number) => call<T.ProcessInfo[]>("list_processes", { serverId }),
  serviceLogs: (serverId: number, unit: string, lines: number) => call<string>("service_logs", { serverId, unit, lines }),
  serviceAction: (serverId: number, unit: string, action: "restart" | "reload", confirmed: boolean) =>
    call<T.ActionResult>("service_action", { serverId, unit, action, confirmed }),

  // website
  websites: (serverId: number) => call<T.Site[]>("websites_list", { serverId }),
  checkUrl: (url: string) => call<T.UrlCheck>("website_check_url", { url }),
  certInfo: (serverId: number, certPath: string) => call<T.CertInfo>("website_cert_info", { serverId, certPath }),
  nginxTest: (serverId: number) => call<T.NginxTest>("nginx_test", { serverId }),
  nginxReload: (serverId: number, confirmed: boolean) => call<T.ActionResult>("nginx_reload", { serverId, confirmed }),

  // file
  filesList: (serverId: number, path: string) => call<T.Listing>("files_list", { serverId, path }),
  filesMkdir: (serverId: number, dir: string, name: string) => call<string>("files_mkdir", { serverId, dir, name }),
  filesRename: (serverId: number, path: string, newName: string) => call<string>("files_rename", { serverId, path, newName }),
  filesDelete: (serverId: number, path: string, confirmed: boolean) => call<void>("files_delete", { serverId, path, confirmed }),
  filesReadText: (serverId: number, path: string) => call<string>("files_read_text", { serverId, path }),
  filesWriteText: (serverId: number, path: string, content: string, confirmed: boolean) =>
    call<void>("files_write_text", { serverId, path, content, confirmed }),
  filesDownload: (serverId: number, path: string, transferId: string) => call<string>("files_download", { serverId, path, transferId }),
  filesUpload: (serverId: number, localPath: string, remoteDir: string, overwrite: boolean, transferId: string) =>
    call<string>("files_upload", { serverId, localPath, remoteDir, overwrite, transferId }),

  // terminal
  terminalOpen: (serverId: number, cols: number, rows: number) => call<string>("terminal_open", { serverId, cols, rows }),
  terminalWrite: (id: string, data: string) => call<void>("terminal_write", { id, data }),
  terminalResize: (id: string, cols: number, rows: number) => call<void>("terminal_resize", { id, cols, rows }),
  terminalClose: (id: string) => call<void>("terminal_close", { id }),

  // database & backup
  dbList: (serverId: number) => call<T.DbInfo[]>("db_list", { serverId }),
  backupDefaultDir: (serverId: number) => call<string>("backup_default_dir", { serverId }),
  backupCreate: (serverId: number, databases: string[], remoteDir: string, download: boolean) =>
    call<T.BackupBatch>("backup_create", { serverId, databases, remoteDir, download }),
  backupHistory: (serverId: number | null) => call<T.BackupRecord[]>("backup_history", { serverId }),
  backupDeleteRecord: (id: number) => call<void>("backup_delete_record", { id }),
  backupRemoteList: (serverId: number, dir: string) => call<T.RemoteBackup[]>("backup_remote_list", { serverId, dir }),
  backupDownload: (serverId: number, backupId: number) => call<string>("backup_download", { serverId, backupId }),
  backupRestore: (serverId: number, remoteFile: string, database: string, confirmName: string) =>
    call<string>("backup_restore", { serverId, remoteFile, database, confirmName }),

  // deployment
  deployProjects: (serverId: number) => call<T.Project[]>("deploy_projects", { serverId }),
  deploySave: (input: T.ProjectInput) => call<T.Project>("deploy_project_save", { input }),
  deployDelete: (id: number) => call<void>("deploy_project_delete", { id }),
  deployGit: (projectId: number, fetch: boolean) => call<T.GitStatus>("deploy_git_status", { projectId, fetch }),
  deployHistory: (projectId: number) => call<T.Deployment[]>("deploy_history", { projectId }),
  deployRun: (projectId: number, confirmMigrations: boolean) => call<T.Deployment>("deploy_run", { projectId, confirmMigrations }),
  deployRollback: (deploymentId: number, confirmMigrations: boolean) =>
    call<T.Deployment>("deploy_rollback", { deploymentId, confirmMigrations }),

  // AI
  aiStatus: () => call<T.AiStatus>("ai_status"),
  aiChat: (messages: T.ChatMessage[]) => call<T.ChatReply>("ai_chat", { messages }),
  aiRun: (serverId: number, command: string, confirmed: boolean) =>
    call<T.CommandResult>("ai_run_command", { serverId, command, confirmed }),

  // log & pengaturan
  logs: (opts: { serverId?: number | null; status?: string; search?: string; limit?: number }) =>
    call<T.LogEntry[]>("logs_list", {
      serverId: opts.serverId ?? null, status: opts.status || null, search: opts.search || null, limit: opts.limit ?? 200,
    }),
  logsClear: () => call<number>("logs_clear"),
  settingsGet: () => call<T.AppSettings>("settings_get"),
  settingsSet: (value: T.AppSettings) => call<void>("settings_set", { value }),
};

export function on<P>(event: string, handler: (payload: P) => void): Promise<UnlistenFn> {
  return listen<P>(event, (e) => handler(e.payload));
}
