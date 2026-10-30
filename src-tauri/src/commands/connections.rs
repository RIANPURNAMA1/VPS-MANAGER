use crate::error::{AppError, Result};
use crate::security::creds;
use crate::ssh::{self, blocking, Conn};
use crate::state::Shared;
use crate::storage::profiles::{self, KnownHost, Profile, ProfileInput};
use crate::storage::{logs, settings};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn list_profiles(state: State<'_, Shared>) -> Result<Vec<Profile>> {
    let st = state.inner().clone();
    blocking(move || profiles::list(&st.db)).await
}

#[tauri::command]
pub async fn save_profile(state: State<'_, Shared>, input: ProfileInput) -> Result<Profile> {
    let st = state.inner().clone();
    blocking(move || {
        let p = profiles::save(&st.db, &input)?;
        // Profil berubah: sesi lama tidak valid lagi.
        if let Some(c) = st.conns.lock().unwrap_or_else(|e| e.into_inner()).remove(&p.id) {
            c.close();
        }
        logs::record(&st.db, Some((p.id, &p.name)), "profile.save", "success", Some(0), 0, "Profil server disimpan");
        Ok(p)
    })
    .await
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, Shared>, id: i64) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        let name = profiles::get(&st.db, id).map(|p| p.name).unwrap_or_default();
        close_conn(&st, id);
        profiles::delete(&st.db, id)?;
        logs::record(&st.db, None, "profile.delete", "success", Some(0), 0, &format!("Profil '{name}' dihapus beserta kredensialnya"));
        Ok(())
    })
    .await
}

fn close_conn(st: &crate::state::AppState, id: i64) {
    if let Some(c) = st.conns.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
        c.close();
    }
    let mut terms = st.terminals.lock().unwrap_or_else(|e| e.into_inner());
    for (_, t) in terms.iter().filter(|(_, t)| t.server_id == id) {
        let _ = t.tx.send(crate::terminal::TermMsg::Close);
    }
    terms.retain(|_, t| t.server_id != id);
}

#[derive(Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub latency_ms: u128,
    pub remote: String,
}

#[tauri::command]
pub async fn test_connection(state: State<'_, Shared>, input: ProfileInput, trust_fingerprint: Option<String>) -> Result<TestResult> {
    let st = state.inner().clone();
    blocking(move || {
        input.validate()?;
        let cfg = settings::load(&st.db)?;
        let p = input.to_profile();
        let secret = match input.secret.as_deref() {
            Some(s) if !s.is_empty() => Some(s.to_string()),
            _ => match input.id {
                Some(id) => creds::get_secret(id)?,
                None => None,
            },
        };
        let t = Instant::now();
        let sess = ssh::open_session(&st.db, &p, secret.as_deref(), cfg.ssh_timeout_secs, trust_fingerprint.as_deref());
        let sess = match sess {
            Ok(s) => s,
            Err(e) => {
                logs::record(&st.db, None, "connection.test", "error", None, t.elapsed().as_millis() as i64, &format!("{}: {}", p.name, e.message));
                return Err(e);
            }
        };
        let latency = t.elapsed().as_millis();
        let mut ch = sess.channel_session()?;
        ch.exec("uname -sr")?;
        let mut out = String::new();
        std::io::Read::read_to_string(&mut ch, &mut out)?;
        let _ = ch.wait_close();
        logs::record(&st.db, None, "connection.test", "success", Some(0), latency as i64, &format!("{} berhasil diuji", p.name));
        Ok(TestResult { ok: true, latency_ms: latency, remote: out.trim().to_string() })
    })
    .await
}

#[tauri::command]
pub async fn connect_profile(state: State<'_, Shared>, id: i64, trust_fingerprint: Option<String>) -> Result<u128> {
    let st = state.inner().clone();
    blocking(move || {
        let p = profiles::get(&st.db, id)?;
        let cfg = settings::load(&st.db)?;
        let t = Instant::now();
        match Conn::connect(&st.db, p.clone(), cfg.ssh_timeout_secs, trust_fingerprint.as_deref()) {
            Ok(c) => {
                let ms = t.elapsed().as_millis();
                st.conns.lock().unwrap_or_else(|e| e.into_inner()).insert(id, Arc::new(c));
                logs::record(&st.db, Some((id, &p.name)), "connection.connect", "success", Some(0), ms as i64, "Terhubung");
                Ok(ms)
            }
            Err(e) => {
                logs::record(&st.db, Some((id, &p.name)), "connection.connect", "error", None, t.elapsed().as_millis() as i64, &e.message);
                Err(e)
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn disconnect_profile(state: State<'_, Shared>, id: i64) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        let name = profiles::get(&st.db, id).map(|p| p.name).unwrap_or_default();
        close_conn(&st, id);
        logs::record(&st.db, Some((id, &name)), "connection.disconnect", "success", Some(0), 0, "Terputus oleh pengguna");
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn connection_states(state: State<'_, Shared>) -> Result<Vec<i64>> {
    let st = state.inner().clone();
    blocking(move || Ok(st.conns.lock().unwrap_or_else(|e| e.into_inner()).keys().copied().collect())).await
}

/// Cek apakah koneksi masih hidup (ping). Jika mati, sesi dibuang agar status akurat.
#[tauri::command]
pub async fn check_connection(state: State<'_, Shared>, id: i64) -> Result<u128> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(id)?;
        match c.ping() {
            Ok(ms) => Ok(ms),
            Err(e) => {
                close_conn(&st, id);
                logs::record(&st.db, Some((id, &c.profile.name)), "connection.lost", "error", None, 0, &e.message);
                Err(AppError::new("connection_lost", "Koneksi terputus. Silakan hubungkan kembali."))
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn list_known_hosts(state: State<'_, Shared>) -> Result<Vec<KnownHost>> {
    let st = state.inner().clone();
    blocking(move || profiles::list_known_hosts(&st.db)).await
}

#[tauri::command]
pub async fn forget_known_host(state: State<'_, Shared>, host: String, port: u16) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        profiles::forget_host(&st.db, &host, port)?;
        logs::record(&st.db, None, "security.forget_host", "success", Some(0), 0, &format!("Host key {host}:{port} dihapus"));
        Ok(())
    })
    .await
}
