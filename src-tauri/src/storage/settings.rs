use super::Db;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub theme: String,
    pub monitor_interval_secs: u64,
    pub ssh_timeout_secs: u64,
    pub download_dir: String,
    pub max_transfer_mb: u64,
    pub ollama_url: String,
    pub ollama_model: String,
    pub log_retention_days: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            monitor_interval_secs: 10,
            ssh_timeout_secs: 15,
            download_dir: String::new(),
            max_transfer_mb: 1024,
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: "llama3.2".into(),
            log_retention_days: 90,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<()> {
        if !["light", "dark", "system"].contains(&self.theme.as_str()) {
            return Err(AppError::invalid("Tema tidak dikenal."));
        }
        if !(3..=3600).contains(&self.monitor_interval_secs) {
            return Err(AppError::invalid("Interval monitoring harus 3–3600 detik."));
        }
        if !(3..=120).contains(&self.ssh_timeout_secs) {
            return Err(AppError::invalid("Timeout SSH harus 3–120 detik."));
        }
        if !(1..=20480).contains(&self.max_transfer_mb) {
            return Err(AppError::invalid("Batas transfer harus 1–20480 MB."));
        }
        if !(1..=3650).contains(&self.log_retention_days) {
            return Err(AppError::invalid("Retensi log harus 1–3650 hari."));
        }
        crate::ai::validate_ollama_url(&self.ollama_url)?;
        if self.ollama_model.trim().is_empty() || self.ollama_model.len() > 100 {
            return Err(AppError::invalid("Nama model Ollama tidak valid."));
        }
        Ok(())
    }
}

pub fn get_raw(db: &Db, key: &str) -> Result<Option<String>> {
    let conn = db.lock();
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key=?1")?;
    let mut rows = stmt.query([key])?;
    Ok(match rows.next()? {
        Some(r) => Some(r.get(0)?),
        None => None,
    })
}

pub fn load(db: &Db) -> Result<AppSettings> {
    let mut s = AppSettings::default();
    if let Some(json) = get_raw(db, "app")? {
        if let Ok(v) = serde_json::from_str::<AppSettings>(&json) {
            s = v;
        }
    }
    Ok(s)
}

pub fn save(db: &Db, s: &AppSettings) -> Result<()> {
    s.validate()?;
    db.lock().execute(
        "INSERT INTO settings (key,value) VALUES ('app',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [serde_json::to_string(s)?],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_valid_and_roundtrip() {
        let db = Db::memory().unwrap();
        let mut s = load(&db).unwrap();
        assert!(s.validate().is_ok());
        s.monitor_interval_secs = 30;
        save(&db, &s).unwrap();
        assert_eq!(load(&db).unwrap().monitor_interval_secs, 30);
    }

    #[test]
    fn rejects_bad_values() {
        let mut s = AppSettings::default();
        s.monitor_interval_secs = 0;
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.ollama_url = "http://evil.example.com:11434".into();
        assert!(s.validate().is_err());
    }
}
