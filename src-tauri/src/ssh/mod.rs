//! Koneksi SSH/SFTP dengan verifikasi host key (TOFU eksplisit: pengguna harus menyetujui fingerprint).
use crate::error::{AppError, Result};
use crate::security::{creds, redact, validate};
use crate::storage::{profiles, profiles::Profile, Db};
use base64::Engine;
use sha2::{Digest, Sha256};
use ssh2::{Session, Sftp};
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

impl ExecOutput {
    pub fn ok(&self) -> bool {
        self.exit == 0
    }
    /// stdout jika sukses; jika gagal, error dengan stderr yang sudah disamarkan.
    pub fn into_result(self, what: &str) -> Result<String> {
        if self.exit == 0 {
            Ok(self.stdout)
        } else {
            Err(AppError::new("command_failed", format!("{what} gagal (exit {}): {}", self.exit, redact::redact_trim(self.stderr.trim(), 400))))
        }
    }
}

pub fn fingerprint(bytes: &[u8]) -> String {
    let h = Sha256::digest(bytes);
    format!("SHA256:{}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(h))
}

fn key_type_name(t: ssh2::HostKeyType) -> String {
    match format!("{t:?}").as_str() {
        "Rsa" => "ssh-rsa",
        "Dss" => "ssh-dss",
        "Ecdsa256" => "ecdsa-sha2-nistp256",
        "Ecdsa384" => "ecdsa-sha2-nistp384",
        "Ecdsa521" => "ecdsa-sha2-nistp521",
        "Ed255219" => "ssh-ed25519",
        _ => "unknown",
    }
    .to_string()
}

pub fn resolve_secret(p: &Profile) -> Result<Option<String>> {
    if p.id > 0 {
        creds::get_secret(p.id)
    } else {
        Ok(None)
    }
}

/// Buka sesi SSH terautentikasi. `trust` = fingerprint yang baru saja disetujui pengguna.
pub fn open_session(db: &Db, p: &Profile, secret: Option<&str>, timeout_secs: u64, trust: Option<&str>) -> Result<Session> {
    let timeout = Duration::from_secs(timeout_secs.max(1));
    let addrs: Vec<_> = (p.host.as_str(), p.port)
        .to_socket_addrs()
        .map_err(|e| AppError::new("dns", format!("Host {} tidak dapat ditemukan: {e}", p.host)))?
        .collect();
    let mut last_err = None;
    let mut tcp = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(s) => {
                tcp = Some(s);
                break;
            }
            Err(e) => last_err = Some(e),
        }
    }
    let tcp = tcp.ok_or_else(|| {
        let why = last_err.map(|e| e.to_string()).unwrap_or_else(|| "tidak ada alamat".into());
        AppError::new("connect_failed", format!("Tidak dapat terhubung ke {}:{} ({why}).", p.host, p.port))
    })?;
    let _ = tcp.set_nodelay(true);

    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.set_timeout((timeout.as_millis().min(u32::MAX as u128)) as u32);
    sess.handshake()?;

    // --- verifikasi host key (tidak boleh dinonaktifkan) ---
    let (key, kt) = sess.host_key().ok_or_else(|| AppError::new("host_key", "Server tidak memberikan host key."))?;
    let fp = fingerprint(key);
    let ktn = key_type_name(kt);
    let info = serde_json::json!({ "host": p.host, "port": p.port, "fingerprint": fp, "key_type": ktn });
    match profiles::known_host(db, &p.host, p.port)? {
        Some(k) if k.fingerprint == fp => {}
        Some(k) => {
            return Err(AppError::new(
                "host_key_changed",
                format!(
                    "PERINGATAN: host key {} berubah. Bisa berarti server di-install ulang atau ada serangan man-in-the-middle. Koneksi dibatalkan.",
                    p.host
                ),
            )
            .with_data(serde_json::json!({
                "host": p.host, "port": p.port, "fingerprint": fp, "key_type": ktn,
                "known_fingerprint": k.fingerprint
            })));
        }
        None => {
            if trust == Some(fp.as_str()) {
                profiles::trust_host(db, &p.host, p.port, &ktn, &fp)?;
            } else {
                return Err(AppError::new(
                    "host_key_unknown",
                    format!("Host {} belum dikenal. Periksa fingerprint sebelum melanjutkan.", p.host),
                )
                .with_data(info));
            }
        }
    }

    // --- autentikasi ---
    match p.auth_type.as_str() {
        "key" => {
            let key_path = p.key_path.as_deref().unwrap_or("");
            if !Path::new(key_path).is_file() {
                return Err(AppError::new("key_missing", format!("File private key tidak ditemukan: {key_path}")));
            }
            sess.userauth_pubkey_file(&p.username, None, Path::new(key_path), secret.filter(|s| !s.is_empty()))
                .map_err(|e| {
                    AppError::new("auth_failed", format!("Autentikasi SSH key gagal ({}). Periksa key, passphrase, dan user.", e.message()))
                })?;
        }
        _ => {
            let pw = secret
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::new("auth_failed", "Password belum diisi/tersimpan untuk profil ini. Ketik ulang password lalu simpan."))?;
            let methods = sess.auth_methods(&p.username).unwrap_or("").to_string();
            let mut last = sess.userauth_password(&p.username, pw).err().map(|e| e.message().to_string());
            if !sess.authenticated() && (methods.is_empty() || methods.contains("keyboard-interactive")) {
                // Sebagian server (PAM) hanya menerima keyboard-interactive.
                last = sess.userauth_keyboard_interactive(&p.username, &mut KbdPassword(pw.to_string())).err().map(|e| e.message().to_string()).or(last);
            }
            if !sess.authenticated() {
                let hint = if methods.is_empty() { String::new() } else { format!(" Metode yang diterima server: {methods}.") };
                return Err(AppError::new(
                    "auth_failed",
                    format!(
                        "Autentikasi password ditolak ({}).{hint} Pastikan user dan password benar (coba ketik ulang password), dan PasswordAuthentication aktif di server.",
                        last.unwrap_or_else(|| "tanpa detail".into())
                    ),
                ));
            }
        }
    }
    if !sess.authenticated() {
        return Err(AppError::new("auth_failed", "Autentikasi ditolak oleh server."));
    }
    sess.set_keepalive(true, 30);
    Ok(sess)
}

struct KbdPassword(String);
impl ssh2::KeyboardInteractivePrompt for KbdPassword {
    fn prompt<'a>(&mut self, _username: &str, _instructions: &str, prompts: &[ssh2::Prompt<'a>]) -> Vec<String> {
        prompts.iter().map(|_| self.0.clone()).collect()
    }
}

fn run_exec(sess: &Session, cmd: &str) -> Result<ExecOutput> {
    let mut ch = sess.channel_session()?;
    ch.exec(cmd)?;
    let stdout = read_capped(&mut ch)?;
    let stderr = read_capped(&mut ch.stderr())?;
    ch.wait_close()?;
    let exit = ch.exit_status()?;
    Ok(ExecOutput { stdout, stderr, exit })
}

fn read_capped(r: &mut impl Read) -> Result<String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if out.len() < MAX_OUTPUT {
                    let take = n.min(MAX_OUTPUT - out.len());
                    out.extend_from_slice(&buf[..take]);
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    let mut s = String::from_utf8_lossy(&out).into_owned();
    if out.len() >= MAX_OUTPUT {
        s.push_str("\n… [output dipotong]");
    }
    Ok(s)
}

/// Koneksi aktif ke satu server: sesi exec persisten + sesi SFTP ter-cache.
pub struct Conn {
    pub profile: Profile,
    pub db: Db,
    pub timeout_secs: u64,
    session: Mutex<Option<Session>>,
    sftp: Mutex<Option<(Session, Sftp)>>,
}

impl Conn {
    pub fn connect(db: &Db, profile: Profile, timeout_secs: u64, trust: Option<&str>) -> Result<Self> {
        let secret = resolve_secret(&profile)?;
        let sess = open_session(db, &profile, secret.as_deref(), timeout_secs, trust)?;
        Ok(Self { profile, db: db.clone(), timeout_secs, session: Mutex::new(Some(sess)), sftp: Mutex::new(None) })
    }

    pub fn new_session(&self) -> Result<Session> {
        let secret = resolve_secret(&self.profile)?;
        open_session(&self.db, &self.profile, secret.as_deref(), self.timeout_secs, None)
    }

    pub fn is_root(&self) -> bool {
        self.profile.username == "root"
    }

    pub fn exec_timeout(&self, cmd: &str, timeout_secs: u64) -> Result<ExecOutput> {
        for attempt in 0..2 {
            let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            if guard.is_none() {
                *guard = Some(self.new_session()?);
            }
            let sess = guard.as_ref().expect("session");
            sess.set_timeout((timeout_secs * 1000).min(u32::MAX as u64) as u32);
            match run_exec(sess, cmd) {
                Ok(o) => return Ok(o),
                Err(e) if attempt == 0 && e.code == "ssh" => {
                    *guard = None; // koneksi putus: sambungkan ulang sekali
                }
                Err(e) => return Err(e),
            }
        }
        Err(AppError::new("ssh", "Koneksi terputus dan gagal tersambung kembali."))
    }

    pub fn exec(&self, cmd: &str) -> Result<ExecOutput> {
        self.exec_timeout(cmd, self.timeout_secs.max(20))
    }

    /// Jalankan skrip bash. `sudo` hanya dipakai bila profil mengaktifkan sudo dan user bukan root.
    /// Memakai `sudo -n` (tidak pernah meminta password).
    pub fn run(&self, script: &str, sudo: bool, timeout_secs: u64) -> Result<ExecOutput> {
        let full = format!("LC_ALL=C; export LC_ALL; {script}");
        let cmd = if sudo && self.profile.use_sudo && !self.is_root() {
            format!("sudo -n bash -c {}", validate::shell_quote(&full))
        } else {
            format!("bash -c {}", validate::shell_quote(&full))
        };
        self.exec_timeout(&cmd, timeout_secs)
    }

    pub fn run_ok(&self, script: &str, sudo: bool, what: &str) -> Result<String> {
        self.run(script, sudo, self.timeout_secs.max(30))?.into_result(what)
    }

    pub fn with_sftp<T>(&self, f: impl Fn(&Sftp) -> Result<T>) -> Result<T> {
        for attempt in 0..2 {
            let mut guard = self.sftp.lock().unwrap_or_else(|e| e.into_inner());
            if guard.is_none() {
                let s = self.new_session()?;
                let sftp = s.sftp()?;
                *guard = Some((s, sftp));
            }
            let (sess, sftp) = guard.as_ref().expect("sftp");
            sess.set_timeout((self.timeout_secs * 1000).min(u32::MAX as u64) as u32);
            match f(sftp) {
                Ok(v) => return Ok(v),
                Err(e) if attempt == 0 && e.code == "ssh" => *guard = None,
                Err(e) => return Err(e),
            }
        }
        Err(AppError::new("ssh", "Sesi SFTP terputus."))
    }

    /// Cek cepat apakah koneksi masih hidup; mengembalikan latensi.
    pub fn ping(&self) -> Result<u128> {
        let t = Instant::now();
        self.exec_timeout("true", 10)?;
        Ok(t.elapsed().as_millis())
    }

    pub fn close(&self) {
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.sftp.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Jalankan tugas blocking di thread pool.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new("internal", format!("Tugas latar belakang gagal: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_format() {
        let f = fingerprint(b"abc");
        assert!(f.starts_with("SHA256:"));
        assert!(!f.ends_with('='));
        assert_eq!(f, fingerprint(b"abc"));
        assert_ne!(f, fingerprint(b"abd"));
    }

    #[test]
    fn exec_output_error_is_redacted() {
        let o = ExecOutput { stdout: String::new(), stderr: "access denied password=hunter2".into(), exit: 1 };
        let e = o.into_result("Perintah").unwrap_err();
        assert!(!e.message.contains("hunter2"));
    }
}
