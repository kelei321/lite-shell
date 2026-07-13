use std::{collections::HashMap, sync::Arc, time::Instant};

use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::ssh::{exec_capture, CommandError, SessionManager};

const MONITOR_COMMAND: &str = "LC_ALL=C; printf '__UPTIME__\\n'; cat /proc/uptime; printf '__CPU__\\n'; head -n 1 /proc/stat; printf '__CORES__\\n'; getconf _NPROCESSORS_ONLN; printf '__LOAD__\\n'; cat /proc/loadavg; printf '__MEM__\\n'; cat /proc/meminfo; printf '__NET__\\n'; cat /proc/net/dev; printf '__DISK__\\n'; df -Pk -x tmpfs -x devtmpfs 2>/dev/null";

#[derive(Default)]
pub struct SystemMonitor {
    previous: Arc<Mutex<HashMap<String, PreviousSample>>>,
}

struct PreviousSample {
    cpu_total: u64,
    cpu_idle: u64,
    network_rx: u64,
    network_tx: u64,
    sampled_at: Instant,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    uptime_seconds: u64,
    cpu_usage_percent: f64,
    cpu_cores: u32,
    load_average: [f64; 3],
    memory_total: u64,
    memory_used: u64,
    swap_total: u64,
    swap_used: u64,
    network_rx_bytes_per_second: u64,
    network_tx_bytes_per_second: u64,
    latency_ms: u64,
    disks: Vec<DiskMetrics>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    path: String,
    total: u64,
    used: u64,
    usage_percent: f64,
}

struct RawSample {
    uptime_seconds: u64,
    cpu_total: u64,
    cpu_idle: u64,
    cpu_cores: u32,
    load_average: [f64; 3],
    memory_total: u64,
    memory_available: u64,
    swap_total: u64,
    swap_free: u64,
    network_rx: u64,
    network_tx: u64,
    disks: Vec<DiskMetrics>,
}

#[tauri::command]
pub async fn system_metrics(
    sessions: State<'_, SessionManager>,
    monitor: State<'_, SystemMonitor>,
    session_id: String,
) -> Result<SystemMetrics, CommandError> {
    let started = Instant::now();
    let output = exec_capture(&sessions, &session_id, MONITOR_COMMAND).await?;
    let latency_ms = started.elapsed().as_millis() as u64;
    let text = String::from_utf8(output)
        .map_err(|error| CommandError::new("MONITOR_ENCODING_FAILED", error.to_string()))?;
    let raw = parse_sample(&text)?;
    let now = Instant::now();
    let mut previous = monitor.previous.lock().await;
    let (cpu_usage_percent, network_rx_bytes_per_second, network_tx_bytes_per_second) = previous
        .get(&session_id)
        .map(|old| {
            let total_delta = raw.cpu_total.saturating_sub(old.cpu_total);
            let idle_delta = raw.cpu_idle.saturating_sub(old.cpu_idle);
            let cpu = if total_delta == 0 {
                0.0
            } else {
                ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0
            };
            let elapsed = now.duration_since(old.sampled_at).as_secs_f64().max(0.001);
            (
                cpu.clamp(0.0, 100.0),
                (raw.network_rx.saturating_sub(old.network_rx) as f64 / elapsed) as u64,
                (raw.network_tx.saturating_sub(old.network_tx) as f64 / elapsed) as u64,
            )
        })
        .unwrap_or((0.0, 0, 0));
    previous.insert(
        session_id,
        PreviousSample {
            cpu_total: raw.cpu_total,
            cpu_idle: raw.cpu_idle,
            network_rx: raw.network_rx,
            network_tx: raw.network_tx,
            sampled_at: now,
        },
    );

    Ok(SystemMetrics {
        uptime_seconds: raw.uptime_seconds,
        cpu_usage_percent,
        cpu_cores: raw.cpu_cores,
        load_average: raw.load_average,
        memory_total: raw.memory_total,
        memory_used: raw.memory_total.saturating_sub(raw.memory_available),
        swap_total: raw.swap_total,
        swap_used: raw.swap_total.saturating_sub(raw.swap_free),
        network_rx_bytes_per_second,
        network_tx_bytes_per_second,
        latency_ms,
        disks: raw.disks,
    })
}

fn parse_sample(output: &str) -> Result<RawSample, CommandError> {
    let sections = split_sections(output);
    let uptime_seconds = section(&sections, "UPTIME")?
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let cpu_values = section(&sections, "CPU")?
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if cpu_values.len() < 4 {
        return Err(CommandError::new(
            "MONITOR_PARSE_FAILED",
            "无法解析 CPU 数据",
        ));
    }
    let cpu_total = cpu_values.iter().sum();
    let cpu_idle = cpu_values[3] + cpu_values.get(4).copied().unwrap_or(0);
    let cpu_cores = section(&sections, "CORES")?.trim().parse().unwrap_or(1);
    let load_values = section(&sections, "LOAD")?
        .split_whitespace()
        .take(3)
        .filter_map(|value| value.parse::<f64>().ok())
        .collect::<Vec<_>>();
    let load_average = [
        load_values.first().copied().unwrap_or(0.0),
        load_values.get(1).copied().unwrap_or(0.0),
        load_values.get(2).copied().unwrap_or(0.0),
    ];
    let memory = parse_memory(section(&sections, "MEM")?);
    let (network_rx, network_tx) = parse_network(section(&sections, "NET")?);

    Ok(RawSample {
        uptime_seconds,
        cpu_total,
        cpu_idle,
        cpu_cores,
        load_average,
        memory_total: memory.get("MemTotal").copied().unwrap_or(0),
        memory_available: memory
            .get("MemAvailable")
            .or_else(|| memory.get("MemFree"))
            .copied()
            .unwrap_or(0),
        swap_total: memory.get("SwapTotal").copied().unwrap_or(0),
        swap_free: memory.get("SwapFree").copied().unwrap_or(0),
        network_rx,
        network_tx,
        disks: parse_disks(section(&sections, "DISK")?),
    })
}

fn split_sections(output: &str) -> HashMap<String, String> {
    let mut sections = HashMap::new();
    let mut current = None::<String>;
    for line in output.lines() {
        if let Some(name) = line
            .strip_prefix("__")
            .and_then(|value| value.strip_suffix("__"))
        {
            current = Some(name.to_owned());
            sections.entry(name.to_owned()).or_insert_with(String::new);
        } else if let Some(name) = current.as_ref() {
            let value = sections.entry(name.clone()).or_insert_with(String::new);
            value.push_str(line);
            value.push('\n');
        }
    }
    sections
}

fn section<'a>(sections: &'a HashMap<String, String>, name: &str) -> Result<&'a str, CommandError> {
    sections
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| CommandError::new("MONITOR_UNSUPPORTED", "服务器不支持 Linux /proc 监控"))
}

fn parse_memory(input: &str) -> HashMap<String, u64> {
    input
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let kilobytes = value.split_whitespace().next()?.parse::<u64>().ok()?;
            Some((key.to_owned(), kilobytes * 1024))
        })
        .collect()
}

fn parse_network(input: &str) -> (u64, u64) {
    input
        .lines()
        .filter_map(|line| {
            let (interface, values) = line.split_once(':')?;
            if interface.trim() == "lo" {
                return None;
            }
            let values = values
                .split_whitespace()
                .filter_map(|value| value.parse::<u64>().ok())
                .collect::<Vec<_>>();
            Some((
                values.first().copied().unwrap_or(0),
                values.get(8).copied().unwrap_or(0),
            ))
        })
        .fold((0, 0), |total, current| {
            (total.0 + current.0, total.1 + current.1)
        })
}

fn parse_disks(input: &str) -> Vec<DiskMetrics> {
    input
        .lines()
        .skip(1)
        .filter_map(|line| {
            let values = line.split_whitespace().collect::<Vec<_>>();
            if values.len() < 6 {
                return None;
            }
            let path = values[5];
            if path.starts_with("/var/lib/docker")
                || path.starts_with("/run")
                || path.starts_with("/dev")
                || path.starts_with("/sys")
            {
                return None;
            }
            let total = values[1].parse::<u64>().ok()? * 1024;
            let used = values[2].parse::<u64>().ok()? * 1024;
            Some(DiskMetrics {
                path: path.to_owned(),
                total,
                used,
                usage_percent: if total == 0 {
                    0.0
                } else {
                    used as f64 / total as f64 * 100.0
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_sample() {
        let sample = "__UPTIME__\n120.5 10\n__CPU__\ncpu 10 0 5 80 5 0 0 0\n__CORES__\n4\n__LOAD__\n0.10 0.20 0.30 1/1 1\n__MEM__\nMemTotal: 1000 kB\nMemAvailable: 400 kB\nSwapTotal: 200 kB\nSwapFree: 150 kB\n__NET__\neth0: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n__DISK__\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/sda1 1000 500 500 50% /\n";
        let parsed = parse_sample(sample).unwrap();
        assert_eq!(parsed.uptime_seconds, 120);
        assert_eq!(parsed.cpu_cores, 4);
        assert_eq!(parsed.memory_total, 1_024_000);
        assert_eq!(parsed.network_tx, 200);
        assert_eq!(parsed.disks.len(), 1);
    }
}
