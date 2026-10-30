pub mod assistant;
pub mod connections;
pub mod data;
pub mod deploy;
pub mod files_term;
pub mod ops;

use crate::error::Result;
use crate::ssh::Conn;
use crate::state::AppState;
use crate::storage::logs;
use std::time::Instant;

/// Catat hasil operasi ke Activity Logs (ringkasan otomatis disamarkan).
pub fn audit<T>(st: &AppState, c: &Conn, action: &str, t0: Instant, r: &Result<T>, ok_summary: &str) {
    let ms = t0.elapsed().as_millis() as i64;
    let srv = Some((c.profile.id, c.profile.name.as_str()));
    match r {
        Ok(_) => logs::record(&st.db, srv, action, "success", Some(0), ms, ok_summary),
        Err(e) => logs::record(&st.db, srv, action, "error", None, ms, &e.message),
    }
}
