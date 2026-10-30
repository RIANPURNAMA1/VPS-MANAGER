use super::audit;
use crate::backup::{self, BackupRecord, DbInfo, RemoteBackup};
use crate::error::{AppError, Result};
use crate::ssh::blocking;
use crate::state::Shared;
use crate::storage::settings;
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn db_list(state: State<'_, Shared>, server_id: i64) -> Result<Vec<DbInfo>> {
    let st = state.inner().clone();
    blocking(move || backup::list_databases(&*st.conn(server_id)?)).await
}

#[tauri::command]
pub async fn backup_default_dir(state: State<'_, Shared>, server_id: i64) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || Ok(backup::default_remote_dir(&*st.conn(server_id)?))).await
}

#[derive(Serialize)]
pub struct BackupBatch {
    pub records: Vec<BackupRecord>,
    pub succeeded: usize,
    pub failed: usize,
}

#[tauri::command]
pub async fn backup_create(app: AppHandle, state: State<'_, Shared>, server_id: i64, databases: Vec<String>, remote_dir: String, download: bool) -> Result<BackupBatch> {
    let st = state.inner().clone();
    blocking(move || {
        if databases.is_empty() {
            return Err(AppError::invalid("Pilih minimal satu database."));
        }
        let c = st.conn(server_id)?;
        let cfg = settings::load(&st.db)?;
        let dl_dir = super::files_term::effective_download_dir(&app, &cfg);
        let mut records = Vec::new();
        for name in &databases {
            let t = Instant::now();
            let r = backup::backup_one(&c, &st.db, name, &remote_dir);
            let mut rec = match r {
                Ok(rec) => rec,
                Err(e) => {
                    audit::<()>(&st, &c, "backup.create", t, &Err(e.clone()), "");
                    return Err(e);
                }
            };
            if rec.verified && download {
                let tid = format!("backup-{}", rec.id);
                match crate::files::download(&app, &c, &rec.remote_path, &dl_dir, cfg.max_transfer_mb * 1_048_576, &tid) {
                    Ok(local) => {
                        let _ = backup::set_local_path(&st.db, rec.id, &local);
                        rec.local_path = Some(local);
                    }
                    Err(e) => {
                        rec.status = "partial".into();
                        rec.note = Some(format!("Backup di server sukses & terverifikasi, tetapi unduhan gagal: {}", e.message));
                        let _ = st.db.lock().execute("UPDATE backups SET status='partial', note=?1 WHERE id=?2", rusqlite::params![rec.note, rec.id]);
                    }
                }
            }
            crate::storage::logs::record(
                &st.db,
                Some((c.profile.id, &c.profile.name)),
                "backup.create",
                if rec.status == "success" { "success" } else { "error" },
                Some(if rec.verified { 0 } else { 1 }),
                t.elapsed().as_millis() as i64,
                &format!("{} → {} ({})", rec.database_name, rec.remote_path, rec.status),
            );
            records.push(rec);
        }
        let succeeded = records.iter().filter(|r| r.status == "success").count();
        Ok(BackupBatch { failed: records.len() - succeeded, succeeded, records })
    })
    .await
}

#[tauri::command]
pub async fn backup_history(state: State<'_, Shared>, server_id: Option<i64>) -> Result<Vec<BackupRecord>> {
    let st = state.inner().clone();
    blocking(move || backup::history(&st.db, server_id)).await
}

#[tauri::command]
pub async fn backup_delete_record(state: State<'_, Shared>, id: i64) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || backup::delete_record(&st.db, id)).await
}

#[tauri::command]
pub async fn backup_remote_list(state: State<'_, Shared>, server_id: i64, dir: String) -> Result<Vec<RemoteBackup>> {
    let st = state.inner().clone();
    blocking(move || backup::remote_list(&*st.conn(server_id)?, &dir)).await
}

#[tauri::command]
pub async fn backup_download(app: AppHandle, state: State<'_, Shared>, server_id: i64, backup_id: i64) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let rec = backup::get_record(&st.db, backup_id)?;
        let cfg = settings::load(&st.db)?;
        let dir = super::files_term::effective_download_dir(&app, &cfg);
        let t = Instant::now();
        let r = crate::files::download(&app, &c, &rec.remote_path, &dir, cfg.max_transfer_mb * 1_048_576, &format!("backup-{backup_id}"));
        if let Ok(p) = &r {
            let _ = backup::set_local_path(&st.db, backup_id, p);
        }
        audit(&st, &c, "backup.download", t, &r, &format!("Mengunduh {}", rec.remote_path));
        r
    })
    .await
}

#[tauri::command]
pub async fn backup_restore(state: State<'_, Shared>, server_id: i64, remote_file: String, database: String, confirm_name: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = backup::restore(&c, &remote_file, &database, &confirm_name);
        audit(&st, &c, "backup.restore", t, &r, &format!("Restore {database} dari {remote_file}"));
        r
    })
    .await
}
