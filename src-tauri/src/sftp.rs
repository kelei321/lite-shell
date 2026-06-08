use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use ssh2::{Session, Sftp};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader, BufWriter},
    net::TcpStream,
    path::Path,
    sync::Arc,
};
use tauri::State;
use uuid::Uuid;

#[derive(Default)]
pub struct SftpState {
    sessions: Mutex<HashMap<String, Arc<SftpHandle>>>,
}

pub struct SftpHandle {
    _session: Session,
    sftp: Mutex<Sftp>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpConnectPayload {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileItem {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[tauri::command]
pub fn sftp_connect(
    state: State<SftpState>,
    payload: SftpConnectPayload,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let session = create_session(&payload).map_err(|error| error.to_string())?;
    let sftp = session
        .sftp()
        .map_err(|error| format!("create sftp failed: {error}"))?;

    state.sessions.lock().insert(
        id.clone(),
        Arc::new(SftpHandle {
            _session: session,
            sftp: Mutex::new(sftp),
        }),
    );

    Ok(id)
}

#[tauri::command]
pub fn sftp_list_dir(
    state: State<SftpState>,
    connection_id: String,
    path: String,
) -> Result<Vec<RemoteFileItem>, String> {
    inner_sftp_list_dir(state, connection_id, path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_close(state: State<SftpState>, connection_id: String) -> Result<(), String> {
    state.sessions.lock().remove(&connection_id);
    Ok(())
}

#[tauri::command]
pub fn sftp_download_file(
    state: State<SftpState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    inner_sftp_download_file(&state, &connection_id, &remote_path, &local_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_upload_file(
    state: State<SftpState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    inner_sftp_upload_file(&state, &connection_id, &local_path, &remote_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_mkdir(
    state: State<SftpState>,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    inner_sftp_mkdir(&state, &connection_id, &path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_delete(
    state: State<SftpState>,
    connection_id: String,
    path: String,
    is_dir: bool,
) -> Result<(), String> {
    inner_sftp_delete(&state, &connection_id, &path, is_dir).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_rename(
    state: State<SftpState>,
    connection_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    inner_sftp_rename(&state, &connection_id, &old_path, &new_path)
        .map_err(|error| error.to_string())
}

fn inner_sftp_list_dir(
    state: State<SftpState>,
    connection_id: String,
    path: String,
) -> Result<Vec<RemoteFileItem>> {
    let handle = get_sftp_handle(&state, &connection_id)?;

    let sftp = handle.sftp.lock();
    let entries = sftp
        .readdir(Path::new(&path))
        .with_context(|| format!("read remote dir failed: {path}"))?;

    let mut items = Vec::new();

    for (entry_path, stat) in entries {
        let name = entry_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();

        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        items.push(RemoteFileItem {
            path: join_remote_path(&path, &name),
            name,
            is_dir: stat.is_dir(),
            size: stat.size.unwrap_or(0),
        });
    }

    items.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(items)
}

fn inner_sftp_download_file(
    state: &SftpState,
    connection_id: &str,
    remote_path: &str,
    local_path: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let mut remote_file = sftp
        .open(Path::new(remote_path))
        .with_context(|| format!("open remote file failed: {remote_path}"))?;
    let local_file = File::create(local_path)
        .with_context(|| format!("create local file failed: {local_path}"))?;
    let mut writer = BufWriter::new(local_file);
    io::copy(&mut remote_file, &mut writer).with_context(|| {
        format!("copy remote file to local failed: {remote_path} -> {local_path}")
    })?;
    Ok(())
}

fn inner_sftp_upload_file(
    state: &SftpState,
    connection_id: &str,
    local_path: &str,
    remote_path: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let local_file =
        File::open(local_path).with_context(|| format!("open local file failed: {local_path}"))?;
    let mut reader = BufReader::new(local_file);
    let sftp = handle.sftp.lock();
    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .with_context(|| format!("create remote file failed: {remote_path}"))?;
    io::copy(&mut reader, &mut remote_file).with_context(|| {
        format!("copy local file to remote failed: {local_path} -> {remote_path}")
    })?;
    Ok(())
}

fn inner_sftp_mkdir(state: &SftpState, connection_id: &str, path: &str) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    sftp.mkdir(Path::new(path), 0o755)
        .with_context(|| format!("create remote directory failed: {path}"))?;
    Ok(())
}

fn inner_sftp_delete(
    state: &SftpState,
    connection_id: &str,
    path: &str,
    is_dir: bool,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();

    if is_dir {
        sftp.rmdir(Path::new(path))
            .with_context(|| format!("delete remote directory failed: {path}"))?;
    } else {
        sftp.unlink(Path::new(path))
            .with_context(|| format!("delete remote file failed: {path}"))?;
    }

    Ok(())
}

fn inner_sftp_rename(
    state: &SftpState,
    connection_id: &str,
    old_path: &str,
    new_path: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    sftp.rename(Path::new(old_path), Path::new(new_path), None)
        .with_context(|| format!("rename remote path failed: {old_path} -> {new_path}"))?;
    Ok(())
}

fn get_sftp_handle(state: &SftpState, connection_id: &str) -> Result<Arc<SftpHandle>> {
    let sessions = state.sessions.lock();
    sessions
        .get(connection_id)
        .cloned()
        .ok_or_else(|| anyhow!("sftp connection not found"))
}

fn create_session(payload: &SftpConnectPayload) -> Result<Session> {
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

fn join_remote_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else if parent.ends_with('/') {
        format!("{parent}{name}")
    } else {
        format!("{parent}/{name}")
    }
}
