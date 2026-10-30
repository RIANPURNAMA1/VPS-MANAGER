//! SQLite lokal untuk metadata (profil, known hosts, log, backup, deployment, pengaturan).
//! Tidak ada password/private key di sini — itu ada di Windows Credential Manager.
pub mod logs;
pub mod profiles;
pub mod settings;

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
        migrate(&conn)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE profiles (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        host TEXT NOT NULL,
        port INTEGER NOT NULL DEFAULT 22,
        username TEXT NOT NULL,
        auth_type TEXT NOT NULL CHECK (auth_type IN ('key','password')),
        key_path TEXT,
        use_sudo INTEGER NOT NULL DEFAULT 0,
        mysql_defaults_file TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE known_hosts (
        host TEXT NOT NULL,
        port INTEGER NOT NULL,
        key_type TEXT NOT NULL,
        fingerprint TEXT NOT NULL,
        trusted_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (host, port)
    );
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE activity_logs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        ts TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
        server_id INTEGER,
        server_name TEXT,
        action TEXT NOT NULL,
        status TEXT NOT NULL,
        exit_code INTEGER,
        duration_ms INTEGER,
        summary TEXT
    );
    CREATE INDEX idx_logs_ts ON activity_logs(ts DESC);
    "#,
    // v2
    r#"
    CREATE TABLE backups (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        server_id INTEGER NOT NULL,
        database_name TEXT NOT NULL,
        remote_path TEXT NOT NULL,
        local_path TEXT,
        size_bytes INTEGER,
        sha256 TEXT,
        status TEXT NOT NULL,
        verified INTEGER NOT NULL DEFAULT 0,
        note TEXT,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
    );
    CREATE TABLE deploy_projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        server_id INTEGER NOT NULL,
        name TEXT NOT NULL,
        path TEXT NOT NULL,
        repo_url TEXT,
        branch TEXT NOT NULL,
        build_steps TEXT NOT NULL DEFAULT '[]',
        restart_service TEXT,
        verify_url TEXT,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
    );
    CREATE TABLE deployments (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES deploy_projects(id) ON DELETE CASCADE,
        kind TEXT NOT NULL DEFAULT 'deploy',
        started_at TEXT NOT NULL,
        finished_at TEXT,
        status TEXT NOT NULL,
        from_commit TEXT,
        to_commit TEXT,
        log TEXT NOT NULL DEFAULT ''
    );
    "#,
];

fn migrate(conn: &Connection) -> Result<()> {
    let current: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_latest() {
        let db = Db::memory().unwrap();
        let v: usize = db.lock().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len());
        // idempotent
        migrate(&db.lock()).unwrap();
    }
}
