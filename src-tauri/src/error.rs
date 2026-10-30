use serde::Serialize;
use serde_json::Value;

/// Error terstruktur yang dikirim ke frontend: `{ code, message, data? }`.
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), data: None }
    }
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }
    pub fn denied(message: impl Into<String>) -> Self {
        Self::new("denied", message)
    }
    pub fn not_connected() -> Self {
        Self::new("not_connected", "Server belum terhubung. Hubungkan server terlebih dahulu.")
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::new("database", format!("Kesalahan database lokal: {e}"))
    }
}

impl From<ssh2::Error> for AppError {
    fn from(e: ssh2::Error) -> Self {
        match e.code() {
            ssh2::ErrorCode::Session(-9) => Self::new("timeout", "Waktu tunggu habis saat berkomunikasi dengan server."),
            ssh2::ErrorCode::SFTP(3) => Self::new("permission_denied", "Izin ditolak oleh server (permission denied)."),
            ssh2::ErrorCode::SFTP(2) => Self::new("not_found", "File atau direktori tidak ditemukan."),
            _ => Self::new("ssh", format!("Kesalahan SSH: {}", e.message())),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        let code = match e.kind() {
            std::io::ErrorKind::PermissionDenied => "permission_denied",
            std::io::ErrorKind::NotFound => "not_found",
            std::io::ErrorKind::TimedOut => "timeout",
            _ => "io",
        };
        Self::new(code, format!("Kesalahan I/O: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("parse", format!("Data tidak valid: {e}"))
    }
}

impl From<ureq::Error> for AppError {
    fn from(e: ureq::Error) -> Self {
        Self::new("http", format!("Permintaan HTTP gagal: {e}"))
    }
}
