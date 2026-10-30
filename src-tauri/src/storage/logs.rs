use super::Db;
use crate::error::Result;
use crate::security::redact::redact_trim;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct LogEntry {
    pub id: i64,
    pub ts: String,
    pub server_id: Option<i64>,
    pub server_name: Option<String>,
    pub action: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
    pub summary: Option<String>,
}

/// Catat aktivitas. Ringkasan selalu disamarkan; kegagalan menulis log tidak menggagalkan operasi.
pub fn record(
    db: &Db,
    server: Option<(i64, &str)>,
    action: &str,
    status: &str,
    exit_code: Option<i32>,
    duration_ms: i64,
    summary: &str,
) {
    let (sid, sname) = match server {
        Some((i, n)) => (Some(i), Some(n.to_string())),
        None => (None, None),
    };
    let _ = db.lock().execute(
        "INSERT INTO activity_logs (server_id,server_name,action,status,exit_code,duration_ms,summary) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![sid, sname, action, status, exit_code, duration_ms, redact_trim(summary, 500)],
    );
}

pub fn list(db: &Db, server_id: Option<i64>, status: Option<&str>, search: Option<&str>, limit: i64) -> Result<Vec<LogEntry>> {
    let conn = db.lock();
    let mut sql = String::from(
        "SELECT id,ts,server_id,server_name,action,status,exit_code,duration_ms,summary FROM activity_logs WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = server_id {
        sql.push_str(" AND server_id=?");
        args.push(Box::new(s));
    }
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        sql.push_str(" AND status=?");
        args.push(Box::new(s.to_string()));
    }
    if let Some(s) = search.filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND (action LIKE ? OR summary LIKE ? OR server_name LIKE ?)");
        let like = format!("%{}%", s.trim().replace('%', ""));
        args.push(Box::new(like.clone()));
        args.push(Box::new(like.clone()));
        args.push(Box::new(like));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    args.push(Box::new(limit.clamp(1, 1000)));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())), |r| {
            Ok(LogEntry {
                id: r.get(0)?,
                ts: r.get(1)?,
                server_id: r.get(2)?,
                server_name: r.get(3)?,
                action: r.get(4)?,
                status: r.get(5)?,
                exit_code: r.get(6)?,
                duration_ms: r.get(7)?,
                summary: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn purge_older_than(db: &Db, days: u32) -> Result<usize> {
    Ok(db.lock().execute(
        "DELETE FROM activity_logs WHERE ts < strftime('%Y-%m-%dT%H:%M:%SZ','now', ?1)",
        [format!("-{days} days")],
    )?)
}

pub fn clear(db: &Db) -> Result<usize> {
    Ok(db.lock().execute("DELETE FROM activity_logs", [])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_redacted_and_filters() {
        let db = Db::memory().unwrap();
        record(&db, Some((1, "A")), "service.restart", "success", Some(0), 12, "ok password=abc123");
        record(&db, None, "settings.save", "error", None, 1, "gagal");
        let all = list(&db, None, None, None, 50).unwrap();
        assert_eq!(all.len(), 2);
        assert!(!all[1].summary.as_deref().unwrap().contains("abc123"));
        assert_eq!(list(&db, None, Some("error"), None, 50).unwrap().len(), 1);
        assert_eq!(list(&db, Some(1), None, None, 50).unwrap().len(), 1);
        assert_eq!(list(&db, None, None, Some("restart"), 50).unwrap().len(), 1);
        assert_eq!(purge_older_than(&db, 30).unwrap(), 0);
        assert_eq!(clear(&db).unwrap(), 2);
    }
}
