use super::audit;
use crate::error::Result;
use crate::monitoring::{self, Metrics};
use crate::services::{self, ActionResult, Overview, ProcessInfo};
use crate::ssh::blocking;
use crate::state::Shared;
use crate::website::{self, CertInfo, NginxTest, Site, UrlCheck};
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn get_metrics(state: State<'_, Shared>, server_id: i64) -> Result<Metrics> {
    let st = state.inner().clone();
    blocking(move || monitoring::collect(&*st.conn(server_id)?)).await
}

#[tauri::command]
pub async fn services_overview(state: State<'_, Shared>, server_id: i64) -> Result<Overview> {
    let st = state.inner().clone();
    blocking(move || services::overview(&*st.conn(server_id)?)).await
}

#[tauri::command]
pub async fn list_processes(state: State<'_, Shared>, server_id: i64) -> Result<Vec<ProcessInfo>> {
    let st = state.inner().clone();
    blocking(move || services::processes(&*st.conn(server_id)?)).await
}

#[tauri::command]
pub async fn service_logs(state: State<'_, Shared>, server_id: i64, unit: String, lines: u32) -> Result<String> {
    let st = state.inner().clone();
    blocking(move || services::logs(&*st.conn(server_id)?, &unit, lines)).await
}

#[tauri::command]
pub async fn service_action(state: State<'_, Shared>, server_id: i64, unit: String, action: String, confirmed: bool) -> Result<ActionResult> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = services::action(&c, &unit, &action, confirmed);
        match &r {
            Ok(a) => {
                let status = if a.exit == 0 && (a.active_after == "active" || a.active_after == "reloading") { "success" } else { "error" };
                crate::storage::logs::record(&st.db, Some((c.profile.id, &c.profile.name)), &format!("service.{action}"), status, Some(a.exit), t.elapsed().as_millis() as i64, &format!("{unit}: {}", a.message));
            }
            Err(_) => audit(&st, &c, &format!("service.{action}"), t, &r, ""),
        }
        r
    })
    .await
}

#[tauri::command]
pub async fn websites_list(state: State<'_, Shared>, server_id: i64) -> Result<Vec<Site>> {
    let st = state.inner().clone();
    blocking(move || website::list(&*st.conn(server_id)?)).await
}

#[tauri::command]
pub async fn website_check_url(url: String) -> Result<UrlCheck> {
    blocking(move || {
        let u = crate::security::validate::http_url(&url)?;
        Ok(website::check_url(&u))
    })
    .await
}

#[tauri::command]
pub async fn website_cert_info(state: State<'_, Shared>, server_id: i64, cert_path: String) -> Result<CertInfo> {
    let st = state.inner().clone();
    blocking(move || website::cert_info(&*st.conn(server_id)?, &cert_path)).await
}

#[tauri::command]
pub async fn nginx_test(state: State<'_, Shared>, server_id: i64) -> Result<NginxTest> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let r = website::nginx_test(&c);
        audit(&st, &c, "nginx.test", t, &r, "nginx -t dijalankan");
        r
    })
    .await
}

/// Reload Nginx: wajib konfirmasi dan `nginx -t` harus lulus lebih dulu.
#[tauri::command]
pub async fn nginx_reload(state: State<'_, Shared>, server_id: i64, confirmed: bool) -> Result<ActionResult> {
    let st = state.inner().clone();
    blocking(move || {
        let c = st.conn(server_id)?;
        let t = Instant::now();
        let test = website::nginx_test(&c)?;
        if !test.ok {
            let e = crate::error::AppError::new("nginx_invalid", format!("Konfigurasi Nginx tidak valid, reload dibatalkan:\n{}", test.output));
            audit::<()>(&st, &c, "nginx.reload", t, &Err(e.clone()), "");
            return Err(e);
        }
        let r = services::action(&c, "nginx", "reload", confirmed);
        audit(&st, &c, "nginx.reload", t, &r, "Nginx di-reload setelah nginx -t lulus");
        r
    })
    .await
}
