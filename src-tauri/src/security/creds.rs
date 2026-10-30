//! Penyimpanan rahasia di Windows Credential Manager (via crate `keyring`).
//! Password / passphrase tidak pernah ditulis ke SQLite, log, atau dikirim balik ke UI.
use crate::error::{AppError, Result};

const SERVICE: &str = "VPS Manager";

fn entry(profile_id: i64) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("profile-{profile_id}"))
        .map_err(|e| AppError::new("credential_store", format!("Penyimpanan kredensial tidak tersedia: {e}")))
}

pub fn set_secret(profile_id: i64, secret: &str) -> Result<()> {
    entry(profile_id)?
        .set_password(secret)
        .map_err(|e| AppError::new("credential_store", format!("Gagal menyimpan kredensial: {e}")))
}

pub fn get_secret(profile_id: i64) -> Result<Option<String>> {
    match entry(profile_id)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::new("credential_store", format!("Gagal membaca kredensial: {e}"))),
    }
}

pub fn has_secret(profile_id: i64) -> bool {
    matches!(get_secret(profile_id), Ok(Some(_)))
}

pub fn delete_secret(profile_id: i64) -> Result<()> {
    match entry(profile_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::new("credential_store", format!("Gagal menghapus kredensial: {e}"))),
    }
}
