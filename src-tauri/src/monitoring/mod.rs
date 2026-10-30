//! Pengumpulan metrik aktual dari /proc dan df di VPS Linux (Ubuntu). Tidak ada data dummy.
use crate::error::{AppError, Result};
use crate::ssh::Conn;
use serde::Serialize;
use std::collections::HashMap;

const SCRIPT: &str = r#"
echo '@@loadavg'; cat /proc/loadavg
echo '@@stat1'; head -n1 /proc/stat
echo '@@net1'; cat /proc/net/dev
sleep 1
echo '@@stat2'; head -n1 /proc/stat
echo '@@net2'; cat /proc/net/dev
echo '@@mem'; grep -E '^(MemTotal|MemAvailable|MemFree|SwapTotal|SwapFree):' /proc/meminfo
echo '@@disk'; df -PkT -x tmpfs -x devtmpfs -x squashfs -x overlay -x efivarfs 2>/dev/null
echo '@@uptime'; cut -d' ' -f1 /proc/uptime
echo '@@os'; (. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")
echo '@@kernel'; uname -r
echo '@@host'; hostname
echo '@@cpus'; nproc
"#;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct DiskInfo {
    pub filesystem: String,
    pub fstype: String,
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub avail_bytes: u64,
    pub used_percent: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct Metrics {
    pub collected_at: String,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_count: u32,
    pub cpu_percent: Option<f64>,
    pub load: [f64; 3],
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disks: Vec<DiskInfo>,
    pub uptime_secs: u64,
    pub net_rx_bps: Option<f64>,
    pub net_tx_bps: Option<f64>,
    pub net_rx_total: u64,
    pub net_tx_total: u64,
}

fn sections(out: &str) -> HashMap<&str, String> {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut cur: Option<&str> = None;
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            cur = Some(name.trim());
            map.entry(name.trim()).or_default();
        } else if let Some(c) = cur {
            map.get_mut(c).unwrap().push(line);
        }
    }
    map.into_iter().map(|(k, v)| (k, v.join("\n"))).collect()
}

fn cpu_times(line: &str) -> Option<(u64, u64)> {
    let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if v.len() < 5 {
        return None;
    }
    let total: u64 = v.iter().take(8).sum();
    let idle = v[3] + v.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

fn net_totals(text: &str) -> (u64, u64) {
    let (mut rx, mut tx) = (0u64, 0u64);
    for line in text.lines().skip(2) {
        let Some((iface, rest)) = line.split_once(':') else { continue };
        let iface = iface.trim();
        if iface == "lo" || iface.starts_with("docker") || iface.starts_with("veth") || iface.starts_with("br-") {
            continue;
        }
        let f: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if f.len() >= 9 {
            rx += f[0];
            tx += f[8];
        }
    }
    (rx, tx)
}

fn meminfo(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            let kb: u64 = v.split_whitespace().next()?.parse().ok()?;
            Some((k.trim().to_string(), kb * 1024))
        })
        .collect()
}

fn disks(text: &str) -> Vec<DiskInfo> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 7 {
                return None;
            }
            let total = f[2].parse::<u64>().ok()? * 1024;
            let used = f[3].parse::<u64>().ok()? * 1024;
            let avail = f[4].parse::<u64>().ok()? * 1024;
            let pct = f[5].trim_end_matches('%').parse::<f64>().unwrap_or_else(|_| if total > 0 { used as f64 * 100.0 / total as f64 } else { 0.0 });
            Some(DiskInfo {
                filesystem: f[0].into(),
                fstype: f[1].into(),
                mount: f[6..].join(" "),
                total_bytes: total,
                used_bytes: used,
                avail_bytes: avail,
                used_percent: pct,
            })
        })
        .filter(|d| d.total_bytes > 0 && !d.mount.starts_with("/snap") && !d.mount.starts_with("/boot/efi"))
        .collect()
}

pub fn parse(out: &str, elapsed_secs: f64) -> Result<Metrics> {
    let s = sections(out);
    let get = |k: &str| s.get(k).map(|x| x.trim().to_string()).unwrap_or_default();
    if get("mem").is_empty() {
        return Err(AppError::new("parse", "Output monitoring tidak lengkap (apakah server Linux?)."));
    }
    let load: Vec<f64> = get("loadavg").split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
    let cpu = match (cpu_times(&get("stat1")), cpu_times(&get("stat2"))) {
        (Some((t1, i1)), Some((t2, i2))) if t2 > t1 => {
            Some(((1.0 - (i2.saturating_sub(i1)) as f64 / (t2 - t1) as f64) * 100.0).clamp(0.0, 100.0))
        }
        _ => None,
    };
    let (rx1, tx1) = net_totals(&get("net1"));
    let (rx2, tx2) = net_totals(&get("net2"));
    let el = elapsed_secs.max(0.5);
    let mem = meminfo(&get("mem"));
    let total = mem.get("MemTotal").copied().unwrap_or(0);
    let avail = mem.get("MemAvailable").copied().or_else(|| mem.get("MemFree").copied()).unwrap_or(0);
    let swap_total = mem.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = mem.get("SwapFree").copied().unwrap_or(0);
    Ok(Metrics {
        collected_at: chrono::Utc::now().to_rfc3339(),
        hostname: get("host"),
        os: get("os"),
        kernel: get("kernel"),
        cpu_count: get("cpus").parse().unwrap_or(0),
        cpu_percent: cpu,
        load: [load.first().copied().unwrap_or(0.0), load.get(1).copied().unwrap_or(0.0), load.get(2).copied().unwrap_or(0.0)],
        mem_total: total,
        mem_used: total.saturating_sub(avail),
        mem_available: avail,
        swap_total,
        swap_used: swap_total.saturating_sub(swap_free),
        disks: disks(&get("disk")),
        uptime_secs: get("uptime").parse::<f64>().unwrap_or(0.0) as u64,
        net_rx_bps: (rx2 >= rx1).then(|| (rx2 - rx1) as f64 / el),
        net_tx_bps: (tx2 >= tx1).then(|| (tx2 - tx1) as f64 / el),
        net_rx_total: rx2,
        net_tx_total: tx2,
    })
}

pub fn collect(conn: &Conn) -> Result<Metrics> {
    let t = std::time::Instant::now();
    let out = conn.run_ok(SCRIPT, false, "Pengumpulan metrik")?;
    parse(&out, t.elapsed().as_secs_f64().min(3.0).max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@@loadavg
0.15 0.20 0.25 1/200 12345
@@stat1
cpu  1000 0 500 8000 100 0 0 0 0 0
@@net1
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 5000 10 0 0 0 0 0 0 5000 10 0 0 0 0 0 0
  eth0: 1000000 100 0 0 0 0 0 0 500000 80 0 0 0 0 0 0
@@stat2
cpu  1100 0 550 8300 100 0 0 0 0 0
@@net2
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 6000 12 0 0 0 0 0 0 6000 12 0 0 0 0 0 0
  eth0: 1002000 110 0 0 0 0 0 0 504000 90 0 0 0 0 0 0
@@mem
MemTotal:        2048000 kB
MemFree:          500000 kB
MemAvailable:    1024000 kB
SwapTotal:        512000 kB
SwapFree:         512000 kB
@@disk
Filesystem     Type 1024-blocks    Used Available Capacity Mounted on
/dev/vda1      ext4    20511312 8000000  11500000      42% /
/dev/vda15     vfat      106858    6186    100672       6% /boot/efi
@@uptime
86400.55
@@os
Ubuntu 24.04.1 LTS
@@kernel
6.8.0-45-generic
@@host
vps-1
@@cpus
2
";

    #[test]
    fn parses_sample() {
        let m = parse(SAMPLE, 1.0).unwrap();
        assert_eq!(m.hostname, "vps-1");
        assert_eq!(m.os, "Ubuntu 24.04.1 LTS");
        assert_eq!(m.cpu_count, 2);
        assert_eq!(m.load, [0.15, 0.20, 0.25]);
        // delta total = 450, idle delta = 300 → 33.3%
        let cpu = m.cpu_percent.unwrap();
        assert!((cpu - 33.33).abs() < 0.1, "{cpu}");
        assert_eq!(m.mem_total, 2048000 * 1024);
        assert_eq!(m.mem_used, (2048000 - 1024000) * 1024);
        assert_eq!(m.swap_used, 0);
        assert_eq!(m.disks.len(), 1);
        assert_eq!(m.disks[0].mount, "/");
        assert!((m.disks[0].used_percent - 42.0).abs() < 0.01);
        assert_eq!(m.uptime_secs, 86400);
        assert_eq!(m.net_rx_bps.unwrap(), 2000.0);
        assert_eq!(m.net_tx_bps.unwrap(), 4000.0);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("hello", 1.0).is_err());
    }
}
