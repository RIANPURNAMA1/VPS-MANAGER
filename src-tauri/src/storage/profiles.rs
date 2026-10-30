use super::Db;
use crate::error::{AppError, Result};
use crate::security::{creds, validate};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
    pub use_sudo: bool,
    pub mysql_defaults_file: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub has_secret: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ProfileInput {
    pub id: Option<i64>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
    pub use_sudo: bool,
    pub mysql_defaults_file: Option<String>,
    /// Password atau passphrase. `None` = pertahankan yang tersimpan, `Some("")` = hapus.
    pub secret: Option<String>,
}

impl ProfileInput {
    pub fn validate(&self) -> Result<()> {
        let n = self.name.trim();
        if n.is_empty() || n.chars().count() > 64 {
            return Err(AppError::invalid("Nama server wajib diisi (maks. 64 karakter)."));
        }
        validate::host(&self.host)?;
        validate::username(&self.username)?;
        if self.port == 0 {
            return Err(AppError::invalid("Port SSH tidak valid."));
        }
        match self.auth_type.as_str() {
            "key" => {
                let k = self.key_path.as_deref().unwrap_or("").trim();
                if k.is_empty() {
                    return Err(AppError::invalid("Pilih file private key untuk autentikasi SSH key."));
                }
            }
            "password" => {}
            _ => return Err(AppError::invalid("Metode autentikasi tidak dikenal.")),
        }
        if let Some(p) = self.mysql_defaults_file.as_deref().filter(|s| !s.trim().is_empty()) {
            validate::remote_path(p)?;
        }
        Ok(())
    }

    pub fn to_profile(&self) -> Profile {
        Profile {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            host: self.host.trim().to_string(),
            port: self.port,
            username: self.username.trim().to_string(),
            auth_type: self.auth_type.clone(),
            key_path: self.key_path.clone().filter(|s| !s.trim().is_empty()),
            use_sudo: self.use_sudo,
            mysql_defaults_file: self.mysql_defaults_file.clone().filter(|s| !s.trim().is_empty()),
            created_at: String::new(),
            has_secret: false,
        }
    }
}

fn from_row(r: &Row) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: r.get(0)?,
        name: r.get(1)?,
        host: r.get(2)?,
        port: r.get::<_, i64>(3)? as u16,
        username: r.get(4)?,
        auth_type: r.get(5)?,
        key_path: r.get(6)?,
        use_sudo: r.get::<_, i64>(7)? != 0,
        mysql_defaults_file: r.get(8)?,
        created_at: r.get(9)?,
        has_secret: false,
    })
}

const COLS: &str = "id,name,host,port,username,auth_type,key_path,use_sudo,mysql_defaults_file,created_at";

pub fn list(db: &Db) -> Result<Vec<Profile>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM profiles ORDER BY name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    drop(conn);
    Ok(rows.into_iter().map(|mut p| { p.has_secret = creds::has_secret(p.id); p }).collect())
}

pub fn get(db: &Db, id: i64) -> Result<Profile> {
    let conn = db.lock();
    let mut p = conn
        .query_row(&format!("SELECT {COLS} FROM profiles WHERE id=?1"), [id], from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::new("not_found", "Profil server tidak ditemukan."),
            e => e.into(),
        })?;
    drop(conn);
    p.has_secret = creds::has_secret(p.id);
    Ok(p)
}

/// Simpan profil (tambah/ubah) dan rahasianya. Mengembalikan profil tersimpan.
pub fn save(db: &Db, input: &ProfileInput) -> Result<Profile> {
    input.validate()?;
    let p = input.to_profile();
    let id = {
        let conn = db.lock();
        match input.id {
            Some(id) => {
                let n = conn.execute(
                    "UPDATE profiles SET name=?1,host=?2,port=?3,username=?4,auth_type=?5,key_path=?6,use_sudo=?7,mysql_defaults_file=?8 WHERE id=?9",
                    params![p.name, p.host, p.port, p.username, p.auth_type, p.key_path, p.use_sudo as i64, p.mysql_defaults_file, id],
                )?;
                if n == 0 {
                    return Err(AppError::new("not_found", "Profil server tidak ditemukan."));
                }
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO profiles (name,host,port,username,auth_type,key_path,use_sudo,mysql_defaults_file) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![p.name, p.host, p.port, p.username, p.auth_type, p.key_path, p.use_sudo as i64, p.mysql_defaults_file],
                )?;
                conn.last_insert_rowid()
            }
        }
    };
    match input.secret.as_deref() {
        Some("") => creds::delete_secret(id)?,
        Some(s) => {
            creds::set_secret(id, s)?;
            if creds::get_secret(id)?.as_deref() != Some(s) {
                return Err(AppError::new("credential_store", "Kredensial tidak tersimpan dengan benar di Windows Credential Manager."));
            }
        }
        None => {}
    }
    get(db, id)
}

pub fn delete(db: &Db, id: i64) -> Result<()> {
    db.lock().execute("DELETE FROM profiles WHERE id=?1", [id])?;
    db.lock().execute("DELETE FROM deploy_projects WHERE server_id=?1", [id])?;
    db.lock().execute("DELETE FROM backups WHERE server_id=?1", [id])?;
    creds::delete_secret(id)?;
    Ok(())
}

// ---- known hosts ----

#[derive(Serialize, Clone, Debug)]
pub struct KnownHost {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    pub trusted_at: String,
}

pub fn known_host(db: &Db, host: &str, port: u16) -> Result<Option<KnownHost>> {
    let conn = db.lock();
    let mut stmt = conn.prepare("SELECT host,port,key_type,fingerprint,trusted_at FROM known_hosts WHERE host=?1 AND port=?2")?;
    let mut rows = stmt.query(params![host, port])?;
    if let Some(r) = rows.next()? {
        return Ok(Some(KnownHost {
            host: r.get(0)?,
            port: r.get::<_, i64>(1)? as u16,
            key_type: r.get(2)?,
            fingerprint: r.get(3)?,
            trusted_at: r.get(4)?,
        }));
    }
    Ok(None)
}

pub fn trust_host(db: &Db, host: &str, port: u16, key_type: &str, fingerprint: &str) -> Result<()> {
    db.lock().execute(
        "INSERT INTO known_hosts (host,port,key_type,fingerprint) VALUES (?1,?2,?3,?4)
         ON CONFLICT(host,port) DO UPDATE SET key_type=excluded.key_type, fingerprint=excluded.fingerprint, trusted_at=datetime('now')",
        params![host, port, key_type, fingerprint],
    )?;
    Ok(())
}

pub fn list_known_hosts(db: &Db) -> Result<Vec<KnownHost>> {
    let conn = db.lock();
    let mut stmt = conn.prepare("SELECT host,port,key_type,fingerprint,trusted_at FROM known_hosts ORDER BY host")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(KnownHost {
                host: r.get(0)?,
                port: r.get::<_, i64>(1)? as u16,
                key_type: r.get(2)?,
                fingerprint: r.get(3)?,
                trusted_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn forget_host(db: &Db, host: &str, port: u16) -> Result<()> {
    db.lock().execute("DELETE FROM known_hosts WHERE host=?1 AND port=?2", params![host, port])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProfileInput {
        ProfileInput {
            id: None,
            name: "Produksi".into(),
            host: "203.0.113.10".into(),
            port: 22,
            username: "ubuntu".into(),
            auth_type: "key".into(),
            key_path: Some("C:/Users/a/.ssh/id_ed25519".into()),
            use_sudo: true,
            mysql_defaults_file: None,
            secret: None,
        }
    }

    #[test]
    fn validates_input() {
        assert!(input().validate().is_ok());
        let mut i = input();
        i.host = "bad host;rm".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.key_path = None;
        assert!(i.validate().is_err());
        let mut i = input();
        i.auth_type = "x".into();
        assert!(i.validate().is_err());
        let mut i = input();
        i.name = "  ".into();
        assert!(i.validate().is_err());
    }

    #[test]
    fn known_hosts_roundtrip() {
        let db = Db::memory().unwrap();
        assert!(known_host(&db, "h", 22).unwrap().is_none());
        trust_host(&db, "h", 22, "ssh-ed25519", "SHA256:abc").unwrap();
        assert_eq!(known_host(&db, "h", 22).unwrap().unwrap().fingerprint, "SHA256:abc");
        trust_host(&db, "h", 22, "ssh-ed25519", "SHA256:def").unwrap();
        assert_eq!(known_host(&db, "h", 22).unwrap().unwrap().fingerprint, "SHA256:def");
        forget_host(&db, "h", 22).unwrap();
        assert!(known_host(&db, "h", 22).unwrap().is_none());
    }
}
