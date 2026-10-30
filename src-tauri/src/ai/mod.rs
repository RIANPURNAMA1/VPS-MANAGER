//! Asisten AI opsional via Ollama lokal (hanya loopback). AI tidak pernah mengeksekusi perintah
//! sendiri: setiap perintah diklasifikasikan (risk) dan butuh klik/konfirmasi pengguna.
use crate::error::{AppError, Result};
use crate::security::redact::{redact, redact_trim};
use crate::security::risk::{classify, Assessment};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

pub fn validate_ollama_url(u: &str) -> Result<()> {
    let u = u.trim();
    let rest = u
        .strip_prefix("http://")
        .or_else(|| u.strip_prefix("https://"))
        .ok_or_else(|| AppError::invalid("URL Ollama harus diawali http:// atau https://."))?;
    let host_port = rest.split('/').next().unwrap_or("");
    let host = if let Some(h) = host_port.strip_prefix('[') {
        h.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if !["127.0.0.1", "localhost", "::1"].contains(&host) {
        return Err(AppError::invalid("Untuk keamanan, Ollama hanya boleh berupa alamat lokal (127.0.0.1 / localhost)."));
    }
    Ok(())
}

fn base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn agent(secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(secs)).build()
}

#[derive(Serialize, Debug, Clone)]
pub struct AiStatus {
    pub available: bool,
    pub models: Vec<String>,
    pub error: Option<String>,
}

pub fn status(url: &str) -> AiStatus {
    if let Err(e) = validate_ollama_url(url) {
        return AiStatus { available: false, models: vec![], error: Some(e.message) };
    }
    match agent(4).get(&format!("{}/api/tags", base(url))).call() {
        Ok(r) => {
            let v: serde_json::Value = r.into_json().unwrap_or_default();
            let models = v["models"].as_array().map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect()).unwrap_or_default();
            AiStatus { available: true, models, error: None }
        }
        Err(e) => AiStatus { available: false, models: vec![], error: Some(format!("Ollama tidak dapat dijangkau: {e}")) },
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Suggestion {
    pub command: String,
    pub assessment: Assessment,
}

#[derive(Serialize, Debug, Clone)]
pub struct ChatReply {
    pub content: String,
    pub suggestions: Vec<Suggestion>,
}

const SYSTEM: &str = "Kamu adalah asisten administrasi server Ubuntu 24.04 (Nginx, PHP-FPM, MySQL, PM2, systemd). \
Jawab dalam Bahasa Indonesia, ringkas dan akurat. Jelaskan penyebab error dari log yang diberikan, jelaskan risiko tindakan, \
dan beri langkah perbaikan bertahap. Usulkan perintah diagnostik baca-saja lebih dulu. Tulis setiap perintah dalam blok ```bash. \
Jangan pernah mengklaim sudah menjalankan perintah. Jangan meminta password, private key, atau token. Jika tidak yakin, katakan tidak yakin.";

static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```(?:bash|sh|shell)?\s*\n(.*?)```").unwrap());

pub fn extract_commands(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cap in FENCE.captures_iter(text) {
        for line in cap[1].lines() {
            let l = line.trim().trim_start_matches("$ ").trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if !out.contains(&l.to_string()) {
                out.push(l.to_string());
            }
        }
    }
    out.truncate(12);
    out
}

pub fn chat(url: &str, model: &str, messages: &[ChatMessage]) -> Result<ChatReply> {
    validate_ollama_url(url)?;
    let mut msgs = vec![serde_json::json!({"role":"system","content":SYSTEM})];
    for m in messages.iter().rev().take(20).rev() {
        if !["user", "assistant"].contains(&m.role.as_str()) {
            continue;
        }
        // Semua konteks disanitasi sebelum meninggalkan aplikasi.
        msgs.push(serde_json::json!({"role": m.role, "content": redact_trim(&m.content, 12000)}));
    }
    let body = serde_json::json!({"model": model, "messages": msgs, "stream": false});
    let resp = agent(180)
        .post(&format!("{}/api/chat", base(url)))
        .send_json(body)
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => AppError::new("ai_model", format!("Model '{model}' belum ada di Ollama. Jalankan: ollama pull {model}")),
            e => AppError::new("ai_unavailable", format!("Ollama tidak merespons: {e}")),
        })?;
    let v: serde_json::Value = resp.into_json().map_err(|e| AppError::new("parse", e.to_string()))?;
    let content = redact(v["message"]["content"].as_str().unwrap_or("").trim());
    if content.is_empty() {
        return Err(AppError::new("ai_empty", "Model tidak memberikan jawaban."));
    }
    let suggestions = extract_commands(&content).into_iter().map(|c| Suggestion { assessment: classify(&c), command: c }).collect();
    Ok(ChatReply { content, suggestions })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::risk::Risk;

    #[test]
    fn only_loopback_urls() {
        assert!(validate_ollama_url("http://127.0.0.1:11434").is_ok());
        assert!(validate_ollama_url("http://localhost:11434/").is_ok());
        assert!(validate_ollama_url("http://[::1]:11434").is_ok());
        assert!(validate_ollama_url("http://192.168.1.5:11434").is_err());
        assert!(validate_ollama_url("http://localhost.evil.com").is_err());
        assert!(validate_ollama_url("ftp://localhost").is_err());
    }

    #[test]
    fn extracts_and_classifies() {
        let t = "Coba:\n```bash\n# cek\nsystemctl status nginx\n$ sudo systemctl restart nginx\nrm -rf /var/www\n```\nselesai";
        let c = extract_commands(t);
        assert_eq!(c.len(), 3);
        assert_eq!(classify(&c[0]).risk, Risk::Safe);
        assert_eq!(classify(&c[1]).risk, Risk::Caution);
        assert_eq!(classify(&c[2]).risk, Risk::Blocked);
    }

    #[test]
    fn no_fence_no_commands() {
        assert!(extract_commands("tidak ada blok kode").is_empty());
    }
}
