//! Penyamaran data sensitif sebelum disimpan ke log atau dikirim ke AI.
use regex::Regex;
use std::sync::LazyLock;

static PRIVATE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)").unwrap());
static KV_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([A-Za-z0-9_-]*(?:password|passwd|pwd|passphrase|secret|token|api[_-]?key|access[_-]?key|authorization))(["']?\s*[=:]\s*)("[^"]*"|'[^']*'|\S+)"#).unwrap()
});
static CLI_PASSWORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(--password=|\s-p)(\S+)").unwrap());
static BEARER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]+").unwrap());
static URL_CRED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*)://[^/\s:@]+:[^/\s@]+@").unwrap());
static LONG_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(ghp_|gho_|github_pat_|sk-|xox[bap]-|AKIA)[A-Za-z0-9_\-]{12,}").unwrap());

pub fn redact(input: &str) -> String {
    let s = PRIVATE_KEY.replace_all(input, "[PRIVATE KEY DISAMARKAN]");
    let s = URL_CRED.replace_all(&s, "$1://[DISAMARKAN]@");
    let s = BEARER.replace_all(&s, "Bearer [DISAMARKAN]");
    let s = KV_SECRET.replace_all(&s, "$1$2[DISAMARKAN]");
    let s = CLI_PASSWORD.replace_all(&s, "$1[DISAMARKAN]");
    let s = LONG_TOKEN.replace_all(&s, "[TOKEN DISAMARKAN]");
    s.into_owned()
}

/// Redact lalu potong ke `max` karakter.
pub fn redact_trim(input: &str, max: usize) -> String {
    let r = redact(input);
    if r.chars().count() <= max {
        return r;
    }
    let cut: String = r.chars().take(max).collect();
    format!("{cut}… [dipotong]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_key_values() {
        assert!(!redact("DB_PASSWORD=hunter2").contains("hunter2"));
        assert!(!redact("password: \"s3cr3t value\"").contains("s3cr3t"));
        assert!(!redact("mysql --password=abc123 db").contains("abc123"));
        assert!(!redact("mysql -pabc123 db").contains("abc123"));
    }

    #[test]
    fn redacts_tokens_and_urls() {
        assert!(!redact("Authorization: Bearer abc.def.ghi").contains("abc.def"));
        assert!(!redact("postgres://user:pass@host/db").contains("pass@"));
        assert!(!redact("ghp_abcdefghijklmnop1234").contains("abcdefghijklmnop"));
    }

    #[test]
    fn redacts_private_keys() {
        let k = "x\n-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\nBBBB\n-----END OPENSSH PRIVATE KEY-----\ny";
        let r = redact(k);
        assert!(!r.contains("AAAA"));
        assert!(r.contains('y'));
    }

    #[test]
    fn keeps_normal_text() {
        assert_eq!(redact("nginx: [emerg] bind() failed"), "nginx: [emerg] bind() failed");
    }

    #[test]
    fn trims() {
        assert!(redact_trim(&"a".repeat(50), 10).contains("dipotong"));
    }
}
