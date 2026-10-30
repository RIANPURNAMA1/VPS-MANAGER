//! Validasi input di backend. Semua nilai yang masuk ke perintah shell remote
//! harus melewati fungsi di sini dan di-quote dengan `shell_quote`.
use crate::error::{AppError, Result};
use regex::Regex;
use std::sync::LazyLock;

static UNIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9@._:-]{1,128}$").unwrap());
static DB_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_$-]{1,64}$").unwrap());
static BRANCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._/-]{0,99}$").unwrap());
static HOST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._:\-\[\]]{1,255}$").unwrap());
static USER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_.-]{0,63}$").unwrap());
static COMMIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{7,64}$").unwrap());

/// Quote satu argumen untuk shell POSIX (single-quote).
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn unit_name(s: &str) -> Result<String> {
    let s = s.trim();
    if !UNIT_RE.is_match(s) {
        return Err(AppError::invalid("Nama service tidak valid."));
    }
    Ok(if s.contains('.') { s.to_string() } else { format!("{s}.service") })
}

pub fn db_name(s: &str) -> Result<String> {
    if !DB_RE.is_match(s) {
        return Err(AppError::invalid("Nama database hanya boleh berisi huruf, angka, _, $ dan - (maks. 64)."));
    }
    Ok(s.to_string())
}

pub fn branch(s: &str) -> Result<String> {
    if !BRANCH_RE.is_match(s) || s.contains("..") || s.ends_with('/') {
        return Err(AppError::invalid("Nama branch tidak valid."));
    }
    Ok(s.to_string())
}

pub fn commit(s: &str) -> Result<String> {
    if !COMMIT_RE.is_match(s) {
        return Err(AppError::invalid("Hash commit tidak valid."));
    }
    Ok(s.to_string())
}

pub fn host(s: &str) -> Result<String> {
    let s = s.trim();
    if !HOST_RE.is_match(s) {
        return Err(AppError::invalid("Hostname atau alamat IP tidak valid."));
    }
    Ok(s.to_string())
}

pub fn username(s: &str) -> Result<String> {
    let s = s.trim();
    if !USER_RE.is_match(s) {
        return Err(AppError::invalid("Username tidak valid."));
    }
    Ok(s.to_string())
}

pub fn http_url(u: &str) -> Result<String> {
    let u = u.trim();
    if u.len() > 2048 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::invalid("URL tidak valid."));
    }
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Err(AppError::invalid("URL harus diawali http:// atau https://."));
    }
    Ok(u.to_string())
}

/// Normalisasi path remote: wajib absolut, tanpa `..`, tanpa karakter kontrol.
pub fn remote_path(p: &str) -> Result<String> {
    if p.is_empty() || p.len() > 4096 {
        return Err(AppError::invalid("Path kosong atau terlalu panjang."));
    }
    if p.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
        return Err(AppError::invalid("Path mengandung karakter terlarang."));
    }
    if !p.starts_with('/') {
        return Err(AppError::invalid("Path harus absolut (diawali /)."));
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => return Err(AppError::invalid("Path tidak boleh mengandung '..' (path traversal).")),
            s => parts.push(s),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

/// Direktori sistem yang tidak boleh dihapus/di-rename lewat aplikasi.
pub fn is_protected(path: &str) -> bool {
    const P: &[&str] = &[
        "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/lib32", "/proc", "/root", "/run", "/sbin",
        "/sys", "/usr", "/var", "/opt", "/srv", "/mnt", "/media", "/snap", "/tmp", "/usr/bin", "/usr/lib",
        "/usr/local", "/var/lib", "/var/log", "/var/www", "/etc/nginx", "/etc/ssh", "/etc/mysql",
    ];
    P.contains(&path)
}

pub fn parent_of(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => path[..i].to_string(),
    }
}

pub fn join_remote(dir: &str, name: &str) -> Result<String> {
    if name.is_empty() || name.contains('/') || name.contains('\0') || name == "." || name == ".." {
        return Err(AppError::invalid("Nama file tidak valid."));
    }
    remote_path(&format!("{}/{}", dir.trim_end_matches('/'), name))
}

/// Nama file lokal aman dari path remote.
pub fn sanitize_filename(name: &str) -> Result<String> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        return Err(AppError::invalid("Nama file tidak valid."));
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
        assert_eq!(shell_quote("x; rm -rf /"), "'x; rm -rf /'");
    }

    #[test]
    fn validates_units() {
        assert_eq!(unit_name("nginx").unwrap(), "nginx.service");
        assert_eq!(unit_name("php8.3-fpm.service").unwrap(), "php8.3-fpm.service");
        assert!(unit_name("nginx; reboot").is_err());
        assert!(unit_name("$(id)").is_err());
        assert!(unit_name("").is_err());
    }

    #[test]
    fn validates_db_and_branch() {
        assert!(db_name("app_db").is_ok());
        assert!(db_name("a;drop").is_err());
        assert!(branch("main").is_ok());
        assert!(branch("feature/x-1").is_ok());
        assert!(branch("--upload-pack=evil").is_err());
        assert!(branch("a..b").is_err());
        assert!(commit("abc1234").is_ok());
        assert!(commit("abc; ls").is_err());
    }

    #[test]
    fn normalizes_paths_and_blocks_traversal() {
        assert_eq!(remote_path("/var//www/./site/").unwrap(), "/var/www/site");
        assert_eq!(remote_path("/").unwrap(), "/");
        assert!(remote_path("/var/www/../../etc").is_err());
        assert!(remote_path("relative/path").is_err());
        assert!(remote_path("/a\nb").is_err());
        assert!(remote_path("/a\0b").is_err());
    }

    #[test]
    fn protects_system_paths() {
        assert!(is_protected("/"));
        assert!(is_protected("/etc"));
        assert!(!is_protected("/var/www/site"));
    }

    #[test]
    fn joins_and_parents() {
        assert_eq!(join_remote("/var/www", "a.txt").unwrap(), "/var/www/a.txt");
        assert_eq!(join_remote("/", "a").unwrap(), "/a");
        assert!(join_remote("/var", "../x").is_err());
        assert!(join_remote("/var", "a/b").is_err());
        assert_eq!(parent_of("/var/www"), "/var");
        assert_eq!(parent_of("/var"), "/");
    }

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("/etc/passwd").unwrap(), "passwd");
        assert_eq!(sanitize_filename("..\\..\\evil.txt").unwrap(), "evil.txt");
        assert_eq!(sanitize_filename("a:b*c.txt").unwrap(), "a_b_c.txt");
        assert!(sanitize_filename("..").is_err());
        assert!(sanitize_filename("/").is_err());
    }

    #[test]
    fn validates_urls() {
        assert!(http_url("https://example.com/health").is_ok());
        assert!(http_url("file:///etc/passwd").is_err());
        assert!(http_url("https://a b").is_err());
    }
}
