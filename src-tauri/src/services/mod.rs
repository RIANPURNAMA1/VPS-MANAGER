//! Deteksi service, proses, log, dan aksi restart/reload (selalu lewat validasi + konfirmasi).
use crate::error::{AppError, Result};
use crate::security::validate;
use crate::ssh::Conn;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ServiceInfo {
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub user: String,
    pub cpu: f64,
    pub mem: f64,
    pub rss_kb: u64,
    pub elapsed: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pm2Process {
    pub name: String,
    pub pid: Option<i64>,
    pub status: String,
    pub cpu: Option<f64>,
    pub memory: Option<u64>,
    pub restarts: Option<i64>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct Detected {
    pub nginx: Option<String>,
    pub database: Option<String>,
    pub php_fpm: Vec<String>,
    pub ssh: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Overview {
    pub services: Vec<ServiceInfo>,
    pub detected: Detected,
    pub pm2_available: bool,
    pub pm2: Vec<Pm2Process>,
}

pub fn parse_services(out: &str) -> Vec<ServiceInfo> {
    out.lines()
        .filter_map(|l| {
            let l = l.trim_start_matches(['●', '*', ' ']);
            let mut it = l.split_whitespace();
            let name = it.next()?;
            if !name.ends_with(".service") {
                return None;
            }
            let (load, active, sub) = (it.next()?, it.next()?, it.next()?);
            Some(ServiceInfo {
                name: name.into(),
                load: load.into(),
                active: active.into(),
                sub: sub.into(),
                description: it.collect::<Vec<_>>().join(" "),
            })
        })
        .collect()
}

static PHP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^php[0-9.]*-fpm\.service$").unwrap());

pub fn detect(services: &[ServiceInfo]) -> Detected {
    let present = |n: &str| services.iter().find(|s| s.name == n && s.load == "loaded").map(|s| s.name.clone());
    Detected {
        nginx: present("nginx.service"),
        database: present("mysql.service").or_else(|| present("mariadb.service")).or_else(|| present("mysqld.service")),
        php_fpm: services.iter().filter(|s| PHP_RE.is_match(&s.name) && s.load == "loaded").map(|s| s.name.clone()).collect(),
        ssh: present("ssh.service").or_else(|| present("sshd.service")),
    }
}

pub fn parse_pm2(json: &str) -> Vec<Pm2Process> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json.trim()) else { return vec![] };
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|p| Pm2Process {
                    name: p["name"].as_str().unwrap_or("?").to_string(),
                    pid: p["pid"].as_i64(),
                    status: p["pm2_env"]["status"].as_str().unwrap_or("unknown").to_string(),
                    cpu: p["monit"]["cpu"].as_f64(),
                    memory: p["monit"]["memory"].as_u64(),
                    restarts: p["pm2_env"]["restart_time"].as_i64(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_ps(out: &str) -> Vec<ProcessInfo> {
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 7 {
                return None;
            }
            Some(ProcessInfo {
                pid: f[0].parse().ok()?,
                user: f[1].into(),
                cpu: f[2].parse().ok()?,
                mem: f[3].parse().ok()?,
                rss_kb: f[4].parse().ok()?,
                elapsed: f[5].into(),
                command: f[6..].join(" "),
            })
        })
        .collect()
}

pub fn overview(conn: &Conn) -> Result<Overview> {
    let out = conn.run_ok(
        "systemctl list-units --type=service --all --no-pager --no-legend --plain 2>/dev/null",
        false,
        "Membaca daftar service",
    )?;
    let services = parse_services(&out);
    let detected = detect(&services);
    let pm2_out = conn.run("command -v pm2 >/dev/null 2>&1 && pm2 jlist 2>/dev/null | sed -n '/^\\[/,$p'", false, 30)?;
    let pm2_available = conn.run("command -v pm2 >/dev/null 2>&1", false, 15).map(|o| o.ok()).unwrap_or(false);
    Ok(Overview { services, detected, pm2_available, pm2: if pm2_available { parse_pm2(&pm2_out.stdout) } else { vec![] } })
}

pub fn processes(conn: &Conn) -> Result<Vec<ProcessInfo>> {
    let out = conn.run_ok(
        "ps -eo pid=,user=,pcpu=,pmem=,rss=,etime=,args= --sort=-pcpu | head -n 40",
        false,
        "Membaca daftar proses",
    )?;
    Ok(parse_ps(&out))
}

pub fn logs(conn: &Conn, unit: &str, lines: u32) -> Result<String> {
    let unit = validate::unit_name(unit)?;
    let n = lines.clamp(10, 1000);
    let script = format!("journalctl -u {} -n {} --no-pager 2>&1", validate::shell_quote(&unit), n);
    let o = conn.run(&script, true, 30)?;
    Ok(o.stdout)
}

#[derive(Serialize, Debug, Clone)]
pub struct ActionResult {
    pub unit: String,
    pub action: String,
    pub exit: i32,
    pub active_after: String,
    pub message: String,
}

pub fn action(conn: &Conn, unit: &str, action: &str, confirmed: bool) -> Result<ActionResult> {
    if !confirmed {
        return Err(AppError::denied("Aksi service memerlukan konfirmasi pengguna."));
    }
    if !["restart", "reload"].contains(&action) {
        return Err(AppError::invalid("Aksi service tidak diizinkan (hanya restart/reload)."));
    }
    let unit = validate::unit_name(unit)?;
    let q = validate::shell_quote(&unit);
    let o = conn.run(&format!("systemctl {action} {q}"), true, 60)?;
    let state = conn.run(&format!("systemctl is-active {q}"), false, 20)?.stdout.trim().to_string();
    let ok = o.exit == 0 && (state == "active" || state == "reloading");
    Ok(ActionResult {
        unit,
        action: action.to_string(),
        exit: o.exit,
        message: if ok {
            format!("Service {action} berhasil dan status sekarang: {state}.")
        } else {
            format!("Aksi {action} belum terverifikasi berhasil (exit {}, status {state}): {}", o.exit, crate::security::redact::redact_trim(o.stderr.trim(), 300))
        },
        active_after: state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNITS: &str = "\
  nginx.service         loaded active running A high performance web server
  mysql.service         loaded active running MySQL Community Server
  php8.3-fpm.service    loaded active running The PHP 8.3 FastCGI Process Manager
● apache2.service       not-found inactive dead apache2.service
  ssh.service           loaded active running OpenBSD Secure Shell server
  cron.service          loaded active running Regular background program processing daemon
";

    #[test]
    fn parses_and_detects() {
        let s = parse_services(UNITS);
        assert_eq!(s.len(), 5 + 1);
        let d = detect(&s);
        assert_eq!(d.nginx.as_deref(), Some("nginx.service"));
        assert_eq!(d.database.as_deref(), Some("mysql.service"));
        assert_eq!(d.php_fpm, vec!["php8.3-fpm.service"]);
        assert_eq!(d.ssh.as_deref(), Some("ssh.service"));
        assert!(s.iter().any(|x| x.name == "apache2.service" && x.load == "not-found"));
    }

    #[test]
    fn detects_mariadb_and_missing() {
        let s = parse_services("  mariadb.service loaded active running MariaDB\n");
        let d = detect(&s);
        assert_eq!(d.database.as_deref(), Some("mariadb.service"));
        assert!(d.nginx.is_none());
        assert!(d.php_fpm.is_empty());
    }

    #[test]
    fn parses_pm2() {
        let j = r#"[{"name":"api","pid":123,"pm2_env":{"status":"online","restart_time":2},"monit":{"cpu":1.5,"memory":52428800}}]"#;
        let p = parse_pm2(j);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].status, "online");
        assert_eq!(p[0].memory, Some(52428800));
        assert!(parse_pm2("not json").is_empty());
    }

    #[test]
    fn parses_ps() {
        let p = parse_ps("  1 root  0.0  0.1  1234 1-02:03:04 /sbin/init splash\n999 www-data 12.5 3.0 5000 00:10 nginx: worker process\n");
        assert_eq!(p.len(), 2);
        assert_eq!(p[1].user, "www-data");
        assert_eq!(p[1].command, "nginx: worker process");
    }
}
