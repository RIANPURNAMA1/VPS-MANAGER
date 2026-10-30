use super::audit;
use crate::error::{AppError, Result};
use crate::files::{self, Listing};
use crate::ssh::blocking;
use crate::state::Shared;
use crate::storage::settings;
use crate::terminal::{self, TermMsg};
use std::time::Instant;
use tauri::{AppHandle, Manager, State};

fn download_dir(app: &AppHandle, cfg: &settings::AppSettings) -> Result<String> {
    if !cfg.download_dir.trim().is_empty() {
        return Ok(cfg.download_dir.clone());
    }
    app.path()
        .download_dir()
        .map(|p| p.join("VPS Manager").to_string_lossy().into_owned())
        .map_err(|_| AppError::new("settings", "Folder unduhan belum diatur. Atur di Pengaturan."))
}

pub fn effective_download_dir(app: &AppHandle, cfg: &settings::AppSettings) -> String {
    download_dir(app, cfg).unwrap_or_default()
}

#[tauri::command]
pub async fn files_list(state: State<'_, Shared>, server_id: i64, path: String) -> Result<Listing> {
    let st = state.inner().clone();
    blocking(move || files::list(&*st.conn(server_id)?, &path)).await
}

#[tauri::command]
pub async fn files_mkdir(state: State<'_, Shared>, server_id: i64, dir: String, name: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = files::mkdir(&c, &dir, &name);
        audit(&st, &c, "file.mkdir", t, &r, &format!("Membuat {dir}/{name}"));
        r
    })
    .await
}

#[tauri::command]
pub async fn files_rename(state: State<'_, Shared>, server_id: i64, path: String, new_name: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = files::rename(&c, &path, &new_name);
        audit(&st, &c, "file.rename", t, &r, &format!("{path} → {new_name}"));
        r
    })
    .await
}

#[tauri::command]
pub async fn files_delete(state: State<'_, Shared>, server_id: i64, path: String, confirmed: bool) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = files::delete(&c, &path, confirmed);
        audit(&st, &c, "file.delete", t, &r, &format!("Menghapus {path}"));
        r
    })
    .await
}

#[tauri::command]
pub async fn files_read_text(state: State<'_, Shared>, server_id: i64, path: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || files::read_text(&*st.conn(server_id)?, &path)).await
}

#[tauri::command]
pub async fn files_write_text(state: State<'_, Shared>, server_id: i64, path: String, content: String, confirmed: bool) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = files::write_text(&c, &path, &content, confirmed);
        audit(&st, &c, "file.edit", t, &r, &format!("Menyimpan {path} ({} byte)", content.len()));
        r
    })
    .await
}

#[tauri::command]
pub async fn files_download(app: AppHandle, state: State<'_, Shared>, server_id: i64, path: String, transfer_id: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let cfg = settings::load(&st.db)?;
        let dir = download_dir(&app, &cfg)?;
        let t = Instant::now();
        let r = files::download(&app, &c, &path, &dir, cfg.max_transfer_mb * 1_048_576, &transfer_id);
        audit(&st, &c, "file.download", t, &r, &format!("Mengunduh {path}"));
        r
    })
    .await
}

#[tauri::command]
pub async fn files_upload(app: AppHandle, state: State<'_, Shared>, server_id: i64, local_path: String, remote_dir: String, overwrite: bool, transfer_id: String) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let cfg = settings::load(&st.db)?;
        let t = Instant::now();
        let r = files::upload(&app, &c, &local_path, &remote_dir, overwrite, cfg.max_transfer_mb * 1_048_576, &transfer_id);
        audit(&st, &c, "file.upload", t, &r, &format!("Mengunggah ke {remote_dir}"));
        r
    })
    .await
}

// ---------------- terminal ----------------

#[tauri::command]
pub async fn terminal_open(app: AppHandle, state: State<'_, Shared>, server_id: i64, cols: u32, rows: u32) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let id = format!("t{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default());
        let t = Instant::now();
        let h = terminal::open(app, st.clone(), &c, id.clone(), cols, rows);
        audit(&st, &c, "terminal.open", t, &h, "Sesi terminal dibuka");
        st.terminals.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), h?);
        Ok(id)
    })
    .await
}

fn send(st: &crate::state::AppState, id: &str, msg: TermMsg) -> Result<()> {
    let terms = st.terminals.lock().unwrap_or_else(|e| e.into_inner());
    let h = terms.get(id).ok_or_else(|| AppError::new("terminal_closed", "Sesi terminal sudah ditutup."))?;
    h.tx.send(msg).map_err(|_| AppError::new("terminal_closed", "Sesi terminal sudah ditutup."))
}

#[tauri::command]
pub fn terminal_write(state: State<'_, Shared>, id: String, data: String) -> Result<()> {
    if data.len() > 64 * 1024 {
        return Err(AppError::invalid("Input terlalu besar."));
    }
    send(&state, &id, TermMsg::Input(data.into_bytes()))
}

#[tauri::command]
pub fn terminal_resize(state: State<'_, Shared>, id: String, cols: u32, rows: u32) -> Result<()> {
    send(&state, &id, TermMsg::Resize(cols, rows))
}

#[tauri::command]
pub fn terminal_close(state: State<'_, Shared>, id: String) -> Result<()> {
    let _ = send(&state, &id, TermMsg::Close);
    state.terminals.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    Ok(())
}
