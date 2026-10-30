use crate::ai::{self, AiStatus, ChatMessage, ChatReply};
use crate::error::{AppError, Result};
use crate::security::redact::redact_trim;
use crate::security::risk::{classify, Risk};
use crate::ssh::blocking;
use crate::state::Shared;
use crate::storage::{logs, settings};
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn ai_status(state: State<'_, Shared>) -> Result<AiStatus> {
    let st = state.inner().clone();
    blocking(move || Ok(ai::status(&settings::load(&st.db)?.ollama_url))).await
}

#[tauri::command]
pub async fn ai_chat(state: State<'_, Shared>, messages: Vec<ChatMessage>) -> Result<ChatReply> {
    let st = state.inner().clone();
    blocking(move || {
        let cfg = settings::load(&st.db)?;
        let t = Instant::now();
        let r = ai::chat(&cfg.ollama_url, &cfg.ollama_model, &messages);
        let (status, msg) = match &r {
            Ok(_) => ("success", "Analisis AI".to_string()),
            Err(e) => ("error", e.message.clone()),
        };
        logs::record(&st.db, None, "ai.chat", status, None, t.elapsed().as_millis() as i64, &msg);
        r
    })
    .await
}

/// Ambil log service lalu (opsional) kirim ke AI — sudah disanitasi di sisi Rust.
#[tauri::command]
pub async fn ai_sanitize(text: String) -> Result<String> {
    Ok(redact_trim(&text, 12000))
}

#[derive(Serialize)]
pub struct CommandResult {
    pub command: String,
    pub risk: Risk,
    pub exit: i32,
    pub output: String,
}

/// Jalankan perintah usulan AI setelah klasifikasi risiko. Blocked → ditolak. Caution → wajib `confirmed`.
#[tauri::command]
pub async fn ai_run_command(_app: AppHandle, state: State<'_, Shared>, server_id: i64, command: String, confirmed: bool) -> Result<CommandResult> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let a = classify(&command);
        let srv = Some((c.profile.id, c.profile.name.as_str()));
        match a.risk {
            Risk::Blocked => {
                logs::record(&st.db, srv, "ai.command", "blocked", None, 0, &format!("{} — {}", command, a.reason));
                return Err(AppError::denied(format!("Perintah ditolak. {}", a.reason)));
            }
            Risk::Caution if !confirmed => return Err(AppError::denied("Perintah berisiko memerlukan konfirmasi eksplisit.")),
            _ => {}
        }
        let t = Instant::now();
        let o = c.run(&command, c.profile.use_sudo, 60)?;
        let output = redact_trim(&format!("{}{}", o.stdout, o.stderr), 20000);
        logs::record(&st.db, srv, "ai.command", if o.exit == 0 { "success" } else { "error" }, Some(o.exit), t.elapsed().as_millis() as i64, &command);
        Ok(CommandResult { command, risk: a.risk, exit: o.exit, output })
    })
    .await
}

// ---------------- logs & settings ----------------

#[tauri::command]
pub async fn logs_list(state: State<'_, Shared>, server_id: Option<i64>, status: Option<String>, search: Option<String>, limit: i64) -> Result<Vec<logs::LogEntry>> {
    let st = state.inner().clone();
    blocking(move || logs::list(&st.db, server_id, status.as_deref(), search.as_deref(), limit)).await
}

#[tauri::command]
pub async fn logs_clear(state: State<'_, Shared>) -> Result<usize> {
    let st = state.inner().clone();
    blocking(move || logs::clear(&st.db)).await
}

#[tauri::command]
pub async fn settings_get(app: AppHandle, state: State<'_, Shared>) -> Result<settings::AppSettings> {
    let st = state.inner().clone();
    blocking(move || {
        let mut s = settings::load(&st.db)?;
        if s.download_dir.trim().is_empty() {
            s.download_dir = super::files_term::effective_download_dir(&app, &s);
        }
        Ok(s)
    })
    .await
}

#[tauri::command]
pub async fn settings_set(state: State<'_, Shared>, value: settings::AppSettings) -> Result<()> {
    let st = state.inner().clone();
    blocking(move || {
        settings::save(&st.db, &value)?;
        logs::purge_older_than(&st.db, value.log_retention_days)?;
        logs::record(&st.db, None, "settings.save", "success", Some(0), 0, "Pengaturan disimpan");
        Ok(())
    })
    .await
}
