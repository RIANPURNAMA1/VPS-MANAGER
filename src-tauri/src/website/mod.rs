//! Website Manager: baca konfigurasi Nginx (read-only), cek HTTP/HTTPS, SSL, validasi `nginx -t`.
//! Tidak pernah menulis konfigurasi.
use crate::error::Result;
use crate::security::validate;
use crate::ssh::Conn;
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
pub struct Site {
    pub server_names: Vec<String>,
    pub root: Option<String>,
    pub listen: Vec<String>,
    pub ssl_certificate: Option<String>,
    pub has_ssl: bool,
    pub source_file: Option<String>,
    pub config: String,
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

/// Parser sederhana untuk output `nginx -T`: mengambil blok `server { ... }`.
pub fn parse_nginx(dump: &str) -> Vec<Site> {
    let mut sites = Vec::new();
    let mut file: Option<String> = None;
    let mut depth: i32 = 0;
    let mut current: Option<(Site, i32, Vec<String>)> = None; // (site, depth at open, raw lines)

    for raw in dump.lines() {
        if let Some(rest) = raw.strip_prefix("# configuration file ") {
            file = Some(rest.trim_end_matches(':').trim().to_string());
            continue;
        }
        let line = strip_comment(raw);
        if line.is_empty() {
            if let Some((_, _, lines)) = current.as_mut() {
                lines.push(raw.to_string());
            }
            continue;
        }
        let opens_server = current.is_none() && (line == "server {" || line == "server{" || (line.starts_with("server") && line.ends_with('{') && line[6..].trim() == "{"));
        if opens_server {
            current = Some((Site { source_file: file.clone(), ..Default::default() }, depth, vec![raw.to_string()]));
        } else if let Some((site, base, lines)) = current.as_mut() {
            lines.push(raw.to_string());
            if depth == *base + 1 {
                let directive = line.trim_end_matches(';').trim();
                let mut parts = directive.split_whitespace();
                match parts.next() {
                    Some("server_name") => site.server_names.extend(parts.map(String::from)),
                    Some("root") => site.root = parts.next().map(|s| s.trim_matches('"').to_string()),
                    Some("listen") => {
                        let v: Vec<&str> = parts.collect();
                        if v.contains(&"ssl") || v.first().is_some_and(|a| a.ends_with(":443") || *a == "443") {
                            site.has_ssl = true;
                        }
                        site.listen.push(v.join(" "));
                    }
                    Some("ssl_certificate") => {
                        site.ssl_certificate = parts.next().map(String::from);
                        site.has_ssl = true;
                    }
                    _ => {}
                }
            }
        }
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if let Some((_, base, _)) = current.as_ref() {
            if depth <= *base {
                let (mut site, _, lines) = current.take().unwrap();
                site.config = lines.join("\n");
                sites.push(site);
            }
        }
    }
    sites
}

pub fn list(conn: &Conn) -> Result<Vec<Site>> {
    let o = conn.run("nginx -T 2>/dev/null", true, 40)?;
    if o.exit != 0 && o.stdout.trim().is_empty() {
        return Err(crate::error::AppError::new(
            "command_failed",
            "Tidak dapat membaca konfigurasi Nginx. Pastikan Nginx terpasang dan user punya akses (aktifkan sudo pada profil bila perlu).",
        ));
    }
    Ok(parse_nginx(&o.stdout))
}

#[derive(Serialize, Debug, Clone)]
pub struct UrlCheck {
    pub url: String,
    pub ok: bool,
    pub status: Option<u16>,
    pub error: Option<String>,
    pub elapsed_ms: u128,
}

pub fn check_url(url: &str) -> UrlCheck {
    let t = std::time::Instant::now();
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(8)).redirects(0).build();
    let r = agent.get(url).call();
    let (status, error) = match r {
        Ok(resp) => (Some(resp.status()), None),
        Err(ureq::Error::Status(c, _)) => (Some(c), None),
        Err(e) => (None, Some(e.to_string())),
    };
    UrlCheck { url: url.into(), ok: status.is_some_and(|s| s < 400), status, error, elapsed_ms: t.elapsed().as_millis() }
}

#[derive(Serialize, Debug, Clone)]
pub struct CertInfo {
    pub not_after: String,
    pub days_left: Option<i64>,
}

pub fn parse_cert_date(s: &str) -> Option<i64> {
    let raw = s.trim().strip_prefix("notAfter=")?.trim().trim_end_matches(" GMT");
    let cleaned = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let dt = chrono::NaiveDateTime::parse_from_str(&cleaned, "%b %d %H:%M:%S %Y").ok()?;
    Some((dt.and_utc() - chrono::Utc::now()).num_days())
}

pub fn cert_info(conn: &Conn, cert_path: &str) -> Result<CertInfo> {
    let p = validate::remote_path(cert_path)?;
    let o = conn.run(&format!("openssl x509 -enddate -noout -in {}", validate::shell_quote(&p)), true, 20)?;
    let line = o.into_result("Membaca sertifikat")?;
    Ok(CertInfo { not_after: line.trim().trim_start_matches("notAfter=").to_string(), days_left: parse_cert_date(&line) })
}

#[derive(Serialize, Debug, Clone)]
pub struct NginxTest {
    pub ok: bool,
    pub output: String,
}

pub fn nginx_test(conn: &Conn) -> Result<NginxTest> {
    let o = conn.run("nginx -t 2>&1", true, 30)?;
    Ok(NginxTest { ok: o.exit == 0, output: format!("{}{}", o.stdout, o.stderr).trim().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "\
# configuration file /etc/nginx/nginx.conf:
user www-data;
http {
    include /etc/nginx/sites-enabled/*;
}
# configuration file /etc/nginx/sites-enabled/app.conf:
server {
    listen 80;
    server_name example.com www.example.com;
    return 301 https://$host$request_uri;
}
server {
    listen 443 ssl;
    server_name example.com www.example.com;
    root /var/www/example/public;
    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    location / {
        root /should/not/win;
        try_files $uri /index.php;
    }
    location ~ \\.php$ {
        fastcgi_pass unix:/run/php/php8.3-fpm.sock;
    }
}
# configuration file /etc/nginx/sites-enabled/other.conf:
server { # komentar
    listen 8080;
    server_name _;
    root /srv/other;
}
";

    #[test]
    fn parses_server_blocks() {
        let s = parse_nginx(DUMP);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].server_names, vec!["example.com", "www.example.com"]);
        assert!(!s[0].has_ssl);
        assert_eq!(s[0].source_file.as_deref(), Some("/etc/nginx/sites-enabled/app.conf"));
        assert!(s[1].has_ssl);
        assert_eq!(s[1].root.as_deref(), Some("/var/www/example/public"));
        assert_eq!(s[1].ssl_certificate.as_deref(), Some("/etc/letsencrypt/live/example.com/fullchain.pem"));
        assert!(s[1].config.contains("fastcgi_pass"));
        assert_eq!(s[2].root.as_deref(), Some("/srv/other"));
        assert_eq!(s[2].listen, vec!["8080"]);
    }

    #[test]
    fn parses_cert_date() {
        let d = parse_cert_date("notAfter=Jan  1 00:00:00 2099 GMT").unwrap();
        assert!(d > 365 * 50);
        assert!(parse_cert_date("garbage").is_none());
    }
}
