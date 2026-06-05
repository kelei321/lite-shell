use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use ssh2::{Channel, Session};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(Default)]
pub struct SshState {
    sessions: Mutex<HashMap<String, Arc<SshHandle>>>,
}

pub struct SshHandle {
    channel: Mutex<Channel>,
    alive: AtomicBool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectPayload {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
    cols: u32,
    rows: u32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshDataPayload {
    id: String,
    data: String,
}

#[tauri::command]
pub fn ssh_connect(
    app: AppHandle,
    state: State<SshState>,
    payload: SshConnectPayload,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let session = create_session(&payload).map_err(|error| error.to_string())?;

    let mut channel = session
        .channel_session()
        .map_err(|error| format!("create channel failed: {error}"))?;

    channel
        .request_pty(
            "xterm-256color",
            None,
            Some((payload.cols, payload.rows, 0, 0)),
        )
        .map_err(|error| format!("request pty failed: {error}"))?;

    channel
        .shell()
        .map_err(|error| format!("start shell failed: {error}"))?;

    session.set_blocking(false);

    let handle = Arc::new(SshHandle {
        channel: Mutex::new(channel),
        alive: AtomicBool::new(true),
    });

    state.sessions.lock().insert(id.clone(), handle.clone());
    spawn_reader(app, id.clone(), handle);

    Ok(id)
}

#[tauri::command]
pub fn ssh_write(state: State<SshState>, id: String, data: String) -> Result<(), String> {
    let sessions = state.sessions.lock();
    let handle = sessions
        .get(&id)
        .ok_or_else(|| "ssh session not found".to_string())?;

    let mut channel = handle.channel.lock();
    channel
        .write_all(data.as_bytes())
        .map_err(|error| format!("ssh write failed: {error}"))?;

    Ok(())
}

#[tauri::command]
pub fn ssh_resize(state: State<SshState>, id: String, cols: u32, rows: u32) -> Result<(), String> {
    let sessions = state.sessions.lock();
    let handle = sessions
        .get(&id)
        .ok_or_else(|| "ssh session not found".to_string())?;

    let mut channel = handle.channel.lock();
    channel
        .request_pty_size(cols, rows, None, None)
        .map_err(|error| format!("resize failed: {error}"))?;

    Ok(())
}

#[tauri::command]
pub fn ssh_close(state: State<SshState>, id: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock();

    if let Some(handle) = sessions.remove(&id) {
        handle.alive.store(false, Ordering::Relaxed);
        let mut channel = handle.channel.lock();
        let _ = channel.close();
    }

    Ok(())
}

fn create_session(payload: &SshConnectPayload) -> Result<Session> {
    let tcp = TcpStream::connect((payload.host.as_str(), payload.port))
        .with_context(|| format!("connect {}:{} failed", payload.host, payload.port))?;

    let mut session = Session::new().context("create ssh session failed")?;
    session.set_tcp_stream(tcp);
    session.handshake().context("ssh handshake failed")?;

    match (&payload.password, &payload.private_key_path) {
        (Some(password), _) if !password.is_empty() => session
            .userauth_password(&payload.username, password)
            .context("password auth failed")?,
        (_, Some(private_key_path)) if !private_key_path.is_empty() => session
            .userauth_pubkey_file(
                &payload.username,
                None,
                Path::new(private_key_path),
                payload.passphrase.as_deref(),
            )
            .context("private key auth failed")?,
        _ => return Err(anyhow!("password or private key is required")),
    }

    if !session.authenticated() {
        return Err(anyhow!("ssh auth failed"));
    }

    Ok(session)
}

fn spawn_reader(app: AppHandle, id: String, handle: Arc<SshHandle>) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];

        while handle.alive.load(Ordering::Relaxed) {
            let read_result = {
                let mut channel = handle.channel.lock();
                channel.read(&mut buffer)
            };

            match read_result {
                Ok(size) if size > 0 => {
                    let data = String::from_utf8_lossy(&buffer[..size]).to_string();
                    let _ = app.emit(
                        "ssh:data",
                        SshDataPayload {
                            id: id.clone(),
                            data,
                        },
                    );
                }
                Ok(_) => thread::sleep(Duration::from_millis(12)),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(12));
                }
                Err(error) => {
                    let _ = app.emit(
                        "ssh:data",
                        SshDataPayload {
                            id: id.clone(),
                            data: format!("\r\n[ssh read error: {error}]\r\n"),
                        },
                    );
                    break;
                }
            }
        }
    });
}
