//! Database & backup MySQL/MariaDB. Tidak ada password di command line: pakai socket auth (sudo)
//! atau `--defaults-file` yang diatur pada profil. Backup baru "berhasil" bila exit code,
//! `gzip -t`, dan penanda "Dump completed" terverifikasi.
use crate::error::{AppError, Result};
use crate::security::validate::{self, shell_quote};
use crate::ssh::Conn;
use crate::storage::Db;
use rusqlite::params;
use serde::Serialize;

const SYSTEM_DBS: &[&str] = &["information_schema", "performance_schema", "mysql", "sys"];

#[derive(Serialize, Debug, Clone)]
pub struct DbInfo {
    pub name: String,
    pub size_bytes: Option<u64>,
    pub is_system: bool,
}

fn mysql_prefix(conn: &Conn, bin: &str) -> String {
    match conn.profile.mysql_defaults_file.as_deref() {
        Some(f) if !f.trim().is_empty() => format!("{bin} --defaults-file={}", shell_quote(f.trim())),
        _ => bin.to_string(),
    }
}

pub fn default_remote_dir(conn: &Conn) -> String {
    if conn.is_root() {
        "/root/vps-manager-backups".into()
    } else {
        format!("/home/{}/vps-manager-backups", conn.profile.username)
    }
}

pub fn list_databases(conn: &Conn) -> Result<Vec<DbInfo>> {
    let m = mysql_prefix(conn, "mysql");
    let names = conn.run_ok(&format!("{m} -N -B -e 'SHOW DATABASES'"), true, "Membaca daftar database")?;
    let sizes_out = conn
        .run(
            &format!("{m} -N -B -e 'SELECT table_schema, COALESCE(SUM(data_length+index_length),0) FROM information_schema.tables GROUP BY table_schema'"),
            true,
            30,
        )
        .map(|o| o.stdout)
        .unwrap_or_default();
    let sizes: std::collections::HashMap<String, u64> = sizes_out
        .lines()
        .filter_map(|l| {
            let (n, s) = l.split_once('\t')?;
            Some((n.to_string(), s.trim().parse().ok()?))
        })
        .collect();
    Ok(names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| DbInfo { name: n.to_string(), size_bytes: sizes.get(n).copied(), is_system: SYSTEM_DBS.contains(&n) })
        .collect())
}

#[derive(Serialize, Debug, Clone)]
pub struct BackupRecord {
    pub id: i64,
    pub server_id: i64,
    pub database_name: String,
    pub remote_path: String,
    pub local_path: Option<String>,
    pub size_bytes: Option<i64>,
    pub sha256: Option<String>,
    pub status: String,
    pub verified: bool,
    pub note: Option<String>,
    pub created_at: String,
}

fn insert(db: &Db, r: &BackupRecord) -> Result<i64> {
    let c = db.lock();
    c.execute(
        "INSERT INTO backups (server_id,database_name,remote_path,local_path,size_bytes,sha256,status,verified,note) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![r.server_id, r.database_name, r.remote_path, r.local_path, r.size_bytes, r.sha256, r.status, r.verified as i64, r.note],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn history(db: &Db, server_id: Option<i64>) -> Result<Vec<BackupRecord>> {
    let c = db.lock();
    let mut stmt = c.prepare(
        "SELECT id,server_id,database_name,remote_path,local_path,size_bytes,sha256,status,verified,note,created_at FROM backups
         WHERE (?1 IS NULL OR server_id=?1) ORDER BY id DESC LIMIT 500",
    )?;
    let rows = stmt
        .query_map([server_id], |r| {
            Ok(BackupRecord {
                id: r.get(0)?,
                server_id: r.get(1)?,
                database_name: r.get(2)?,
                remote_path: r.get(3)?,
                local_path: r.get(4)?,
                size_bytes: r.get(5)?,
                sha256: r.get(6)?,
                status: r.get(7)?,
                verified: r.get::<_, i64>(8)? != 0,
                note: r.get(9)?,
                created_at: r.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete_record(db: &Db, id: i64) -> Result<()> {
    db.lock().execute("DELETE FROM backups WHERE id=?1", [id])?;
    Ok(())
}

pub fn get_record(db: &Db, id: i64) -> Result<BackupRecord> {
    history(db, None)?.into_iter().find(|b| b.id == id).ok_or_else(|| AppError::new("not_found", "Catatan backup tidak ditemukan."))
}

pub fn set_local_path(db: &Db, id: i64, path: &str) -> Result<()> {
    db.lock().execute("UPDATE backups SET local_path=?1 WHERE id=?2", params![path, id])?;
    Ok(())
}

/// Backup satu database; selalu mengembalikan catatan (status success/failed) dan menyimpannya.
pub fn backup_one(conn: &Conn, db: &Db, database: &str, remote_dir: &str) -> Result<BackupRecord> {
    let database = validate::db_name(database)?;
    let dir = validate::remote_path(remote_dir)?;
    let stamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let file = format!("{dir}/{database}_{stamp}.sql.gz");
    let (qd, qf, qdb) = (shell_quote(&dir), shell_quote(&file), shell_quote(&database));
    let dump = mysql_prefix(conn, "mysqldump");
    let script = format!(
        "set -o pipefail; umask 077; [ ! -e {qf} ] || {{ echo 'File backup sudah ada' >&2; exit 3; }}; \
         mkdir -p {qd} && {dump} --single-transaction --quick --routines --triggers --events {qdb} | gzip -c > {qf}.part && mv {qf}.part {qf}"
    );
    let mut rec = BackupRecord {
        id: 0,
        server_id: conn.profile.id,
        database_name: database.clone(),
        remote_path: file.clone(),
        local_path: None,
        size_bytes: None,
        sha256: None,
        status: "failed".into(),
        verified: false,
        note: None,
        created_at: String::new(),
    };
    let o = conn.run(&script, true, 3600)?;
    if o.exit != 0 {
        let _ = conn.run(&format!("rm -f -- {qf}.part"), false, 20);
        rec.note = Some(format!("mysqldump gagal (exit {}): {}", o.exit, crate::security::redact::redact_trim(o.stderr.trim(), 300)));
        rec.id = insert(db, &rec)?;
        return Ok(rec);
    }
    // Verifikasi: gzip utuh, penanda Dump completed, ukuran, checksum.
    let verify = format!(
        "gzip -t {qf} && zcat {qf} | tail -n 3 | grep -q 'Dump completed' && stat -c %s {qf} && sha256sum {qf} | cut -d' ' -f1"
    );
    let v = conn.run(&verify, false, 600)?;
    if v.exit != 0 {
        rec.note = Some("Backup dibuat tetapi verifikasi gagal (arsip rusak atau dump tidak lengkap). Jangan dipakai.".into());
        rec.id = insert(db, &rec)?;
        return Ok(rec);
    }
    let mut lines = v.stdout.lines();
    rec.size_bytes = lines.next().and_then(|s| s.trim().parse().ok());
    rec.sha256 = lines.next().map(|s| s.trim().to_string());
    rec.verified = true;
    rec.status = "success".into();
    rec.id = insert(db, &rec)?;
    Ok(rec)
}

#[derive(Serialize, Debug, Clone)]
pub struct RemoteBackup {
    pub path: String,
    pub size_bytes: u64,
    pub modified: i64,
}

pub fn remote_list(conn: &Conn, dir: &str) -> Result<Vec<RemoteBackup>> {
    let dir = validate::remote_path(dir)?;
    let o = conn.run(
        &format!("find {} -maxdepth 1 -type f -name '*.sql.gz' -printf '%s\\t%T@\\t%p\\n' 2>/dev/null | sort -k2 -nr", shell_quote(&dir)),
        false,
        30,
    )?;
    Ok(o.stdout
        .lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            Some(RemoteBackup {
                size_bytes: p.next()?.parse().ok()?,
                modified: p.next()?.parse::<f64>().ok()? as i64,
                path: p.next()?.to_string(),
            })
        })
        .collect())
}

/// Restore: wajib konfirmasi dengan mengetik nama database, arsip diuji dulu, DB harus sudah ada.
pub fn restore(conn: &Conn, remote_file: &str, database: &str, confirm_name: &str) -> Result<String> {
    let database = validate::db_name(database)?;
    if confirm_name != database {
        return Err(AppError::denied("Konfirmasi tidak cocok. Ketik nama database persis untuk melanjutkan restore."));
    }
    let file = validate::remote_path(remote_file)?;
    if !file.ends_with(".sql.gz") {
        return Err(AppError::invalid("Hanya file .sql.gz yang didukung untuk restore."));
    }
    let existing = list_databases(conn)?;
    if !existing.iter().any(|d| d.name == database) {
        return Err(AppError::invalid("Database tujuan belum ada. Buat database kosong terlebih dahulu."));
    }
    if existing.iter().any(|d| d.is_system && d.name == database) {
        return Err(AppError::denied("Restore ke database sistem tidak diizinkan."));
    }
    let (qf, qdb) = (shell_quote(&file), shell_quote(&database));
    let t = conn.run(&format!("gzip -t {qf}"), false, 600)?;
    if t.exit != 0 {
        return Err(AppError::new("verify_failed", "Arsip backup rusak (gzip -t gagal). Restore dibatalkan."));
    }
    let m = mysql_prefix(conn, "mysql");
    let o = conn.run(&format!("set -o pipefail; gunzip -c {qf} | {m} {qdb}"), true, 3600)?;
    o.into_result("Restore database")?;
    Ok(format!("Restore {database} dari {file} selesai (exit 0)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_roundtrip() {
        let db = Db::memory().unwrap();
        let rec = BackupRecord {
            id: 0,
            server_id: 1,
            database_name: "app".into(),
            remote_path: "/x/app.sql.gz".into(),
            local_path: None,
            size_bytes: Some(10),
            sha256: Some("abc".into()),
            status: "success".into(),
            verified: true,
            note: None,
            created_at: String::new(),
        };
        let id = insert(&db, &rec).unwrap();
        assert_eq!(history(&db, Some(1)).unwrap().len(), 1);
        assert_eq!(history(&db, Some(2)).unwrap().len(), 0);
        set_local_path(&db, id, "C:/b/app.sql.gz").unwrap();
        assert_eq!(get_record(&db, id).unwrap().local_path.as_deref(), Some("C:/b/app.sql.gz"));
        delete_record(&db, id).unwrap();
        assert!(get_record(&db, id).is_err());
    }
}
