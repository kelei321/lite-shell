use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::{
    collections::HashMap,
    io::Read,
    net::TcpStream,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorConnectPayload {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    hostname: String,
    os: String,
    kernel: String,
    uptime: String,
    cpu_usage: f64,
    memory: MemorySnapshot,
    swap: MemorySnapshot,
    disks: Vec<DiskSnapshot>,
    networks: Vec<NetworkSnapshot>,
    collected_at: u64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    total_mb: u64,
    used_mb: u64,
    usage_percent: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSnapshot {
    mount: String,
    filesystem: String,
    total: String,
    used: String,
    available: String,
    usage_percent: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    name: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

#[tauri::command]
pub fn monitor_snapshot(payload: MonitorConnectPayload) -> Result<MonitorSnapshot, String> {
    inner_monitor_snapshot(payload).map_err(|error| error.to_string())
}

fn inner_monitor_snapshot(payload: MonitorConnectPayload) -> Result<MonitorSnapshot> {
    let session = create_session(&payload).map_err(|_| anyhow!("monitor ssh connection failed"))?;
    let output = run_monitor_script(&session).context("monitor command execution failed")?;
    let sections = parse_sections(&output);

    Ok(MonitorSnapshot {
        hostname: first_line(&sections, "__HOSTNAME__"),
        os: parse_os_release(section_text(&sections, "__OS_RELEASE__")),
        kernel: first_line(&sections, "__UNAME__"),
        uptime: first_line(&sections, "__UPTIME__"),
        cpu_usage: parse_cpu_usage(
            section_text(&sections, "__PROC_STAT_1__"),
            section_text(&sections, "__PROC_STAT_2__"),
        ),
        memory: parse_memory(section_text(&sections, "__FREE__"), "Mem:"),
        swap: parse_memory(section_text(&sections, "__FREE__"), "Swap:"),
        disks: parse_disks(section_text(&sections, "__DF__")),
        networks: parse_networks(section_text(&sections, "__NET_DEV__")),
        collected_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
    })
}

fn create_session(payload: &MonitorConnectPayload) -> Result<Session> {
    let tcp = TcpStream::connect((payload.host.as_str(), payload.port))
        .context("monitor tcp connect failed")?;

    let mut session = Session::new().context("create monitor ssh session failed")?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .context("monitor ssh handshake failed")?;

    match (&payload.password, &payload.private_key_path) {
        (Some(password), _) if !password.is_empty() => session
            .userauth_password(&payload.username, password)
            .context("monitor password auth failed")?,
        (_, Some(private_key_path)) if !private_key_path.is_empty() => session
            .userauth_pubkey_file(
                &payload.username,
                None,
                Path::new(private_key_path),
                payload.passphrase.as_deref(),
            )
            .context("monitor private key auth failed")?,
        _ => return Err(anyhow!("monitor credential is required")),
    }

    if !session.authenticated() {
        return Err(anyhow!("monitor ssh auth failed"));
    }

    Ok(session)
}

fn run_monitor_script(session: &Session) -> Result<String> {
    let script = r#"
printf '__HOSTNAME__\n'
hostname
printf '__UNAME__\n'
uname -srmo
printf '__OS_RELEASE__\n'
cat /etc/os-release 2>/dev/null || true
printf '__UPTIME__\n'
uptime -p 2>/dev/null || uptime
printf '__PROC_STAT_1__\n'
cat /proc/stat
sleep 0.2
printf '__PROC_STAT_2__\n'
cat /proc/stat
printf '__FREE__\n'
free -m
printf '__DF__\n'
df -hP
printf '__NET_DEV__\n'
cat /proc/net/dev
"#;

    let mut channel = session
        .channel_session()
        .context("create monitor channel failed")?;
    channel.exec(script).context("run monitor script failed")?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .context("read monitor output failed")?;
    channel
        .wait_close()
        .context("close monitor channel failed")?;

    let status = channel.exit_status().unwrap_or(0);
    if status != 0 {
        return Err(anyhow!("monitor script exited with status {status}"));
    }

    Ok(output)
}

fn parse_sections(output: &str) -> HashMap<String, String> {
    let mut sections = HashMap::new();
    let mut current = String::new();

    for line in output.lines() {
        if line.starts_with("__") && line.ends_with("__") {
            current = line.trim().to_string();
            sections.entry(current.clone()).or_insert_with(String::new);
            continue;
        }

        if !current.is_empty() {
            let entry = sections.entry(current.clone()).or_insert_with(String::new);
            entry.push_str(line);
            entry.push('\n');
        }
    }

    sections
}

fn section_text<'a>(sections: &'a HashMap<String, String>, key: &str) -> &'a str {
    sections.get(key).map(String::as_str).unwrap_or("")
}

fn first_line(sections: &HashMap<String, String>, key: &str) -> String {
    section_text(sections, key)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn parse_os_release(text: &str) -> String {
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
            return value.trim_matches('"').to_string();
        }
    }

    "-".to_string()
}

fn parse_cpu_usage(first: &str, second: &str) -> f64 {
    let Some(first_cpu) = parse_cpu_line(first) else {
        return 0.0;
    };
    let Some(second_cpu) = parse_cpu_line(second) else {
        return 0.0;
    };

    let total_delta = second_cpu.total.saturating_sub(first_cpu.total);
    let idle_delta = second_cpu.idle_all.saturating_sub(first_cpu.idle_all);

    if total_delta == 0 {
        return 0.0;
    }

    clamp_percent(100.0 * (1.0 - idle_delta as f64 / total_delta as f64))
}

struct CpuSample {
    total: u64,
    idle_all: u64,
}

fn parse_cpu_line(text: &str) -> Option<CpuSample> {
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();

    if values.len() < 8 {
        return None;
    }

    let idle_all = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total = values.iter().take(8).sum();

    Some(CpuSample { total, idle_all })
}

fn parse_memory(text: &str, prefix: &str) -> MemorySnapshot {
    let Some(line) = text
        .lines()
        .find(|line| line.trim_start().starts_with(prefix))
    else {
        return MemorySnapshot::default();
    };

    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();

    let total_mb = values.first().copied().unwrap_or(0);
    let used_mb = values.get(1).copied().unwrap_or(0);

    MemorySnapshot {
        total_mb,
        used_mb,
        usage_percent: percent(used_mb, total_mb),
    }
}

fn parse_disks(text: &str) -> Vec<DiskSnapshot> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let columns = line.split_whitespace().collect::<Vec<_>>();
            if columns.len() < 6 {
                return None;
            }

            Some(DiskSnapshot {
                filesystem: columns[0].to_string(),
                total: columns[1].to_string(),
                used: columns[2].to_string(),
                available: columns[3].to_string(),
                usage_percent: parse_percent(columns[4]),
                mount: columns[5].to_string(),
            })
        })
        .collect()
}

fn parse_networks(text: &str) -> Vec<NetworkSnapshot> {
    text.lines()
        .filter_map(|line| {
            let (name, data) = line.split_once(':')?;
            let name = name.trim();
            if name.is_empty() || name == "lo" {
                return None;
            }

            let values = data
                .split_whitespace()
                .filter_map(|value| value.parse::<u64>().ok())
                .collect::<Vec<_>>();

            Some(NetworkSnapshot {
                name: name.to_string(),
                rx_bytes: values.first().copied().unwrap_or(0),
                tx_bytes: values.get(8).copied().unwrap_or(0),
            })
        })
        .collect()
}

fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }

    clamp_percent(used as f64 * 100.0 / total as f64)
}

fn parse_percent(value: &str) -> f64 {
    value
        .trim_end_matches('%')
        .parse::<f64>()
        .map(clamp_percent)
        .unwrap_or(0.0)
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}
