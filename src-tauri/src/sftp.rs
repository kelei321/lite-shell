use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use ssh2::{FileStat, Session, Sftp};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

const TRANSFER_PROGRESS_EVENT: &str = "sftp-transfer-progress";
const TRANSFER_BUFFER_SIZE: usize = 64 * 1024;
const TRANSFER_PROGRESS_STEP_BYTES: u64 = 512 * 1024;

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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileItem {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileStat {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    permissions: String,
    uid: Option<u32>,
    gid: Option<u32>,
    modified_at: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemotePathInput {
    path: String,
    is_dir: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SftpTransferProgressPayload {
    transfer_id: String,
    transferred_bytes: u64,
    total_bytes: u64,
    percent: f64,
    status: String,
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
pub fn sftp_stat(
    state: State<SftpState>,
    connection_id: String,
    path: String,
) -> Result<RemoteFileStat, String> {
    inner_sftp_stat(&state, &connection_id, &path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_close(state: State<SftpState>, connection_id: String) -> Result<(), String> {
    state.sessions.lock().remove(&connection_id);
    Ok(())
}

#[tauri::command]
pub fn sftp_download_file(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_download_file(
        &app,
        &state,
        &connection_id,
        &remote_path,
        &local_path,
        &transfer_id,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_download_dir(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    remote_path: String,
    local_dir: String,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_download_dir(
        &app,
        &state,
        &connection_id,
        &remote_path,
        &local_dir,
        &transfer_id,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_upload_file(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_upload_file(
        &app,
        &state,
        &connection_id,
        &local_path,
        &remote_path,
        &transfer_id,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_upload_dir(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_upload_dir(
        &app,
        &state,
        &connection_id,
        &local_path,
        &remote_path,
        &transfer_id,
    )
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
pub fn sftp_create_file(
    state: State<SftpState>,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    inner_sftp_create_file(&state, &connection_id, &path).map_err(|error| error.to_string())
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
pub fn sftp_delete_many(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    items: Vec<RemotePathInput>,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_delete_many(&app, &state, &connection_id, &items, &transfer_id)
        .map_err(|error| error.to_string())
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

#[tauri::command]
pub fn sftp_paste(
    app: AppHandle,
    state: State<SftpState>,
    connection_id: String,
    items: Vec<RemoteFileItem>,
    target_path: String,
    mode: String,
    transfer_id: String,
) -> Result<(), String> {
    inner_sftp_paste(
        &app,
        &state,
        &connection_id,
        &items,
        &target_path,
        &mode,
        &transfer_id,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sftp_chmod(
    state: State<SftpState>,
    connection_id: String,
    path: String,
    mode: String,
) -> Result<(), String> {
    inner_sftp_chmod(&state, &connection_id, &path, &mode).map_err(|error| error.to_string())
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

fn inner_sftp_stat(
    state: &SftpState,
    connection_id: &str,
    path: &str,
) -> Result<RemoteFileStat> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let stat = sftp
        .stat(Path::new(path))
        .with_context(|| format!("stat remote path failed: {path}"))?;
    let name = remote_name(path);

    Ok(RemoteFileStat {
        name,
        path: path.to_string(),
        is_dir: stat.is_dir(),
        size: stat.size.unwrap_or(0),
        permissions: format_permissions(stat.perm),
        uid: stat.uid,
        gid: stat.gid,
        modified_at: stat.mtime,
    })
}

fn inner_sftp_download_file(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    remote_path: &str,
    local_path: &str,
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let total_bytes = sftp
        .stat(Path::new(remote_path))
        .ok()
        .and_then(|stat| stat.size)
        .unwrap_or(0);
    let mut remote_file = sftp
        .open(Path::new(remote_path))
        .with_context(|| format!("open remote file failed: {remote_path}"))?;
    let mut local_file = File::create(local_path)
        .with_context(|| format!("create local file failed: {local_path}"))?;

    emit_transfer_progress(app, transfer_id, 0, total_bytes, "running");
    copy_with_progress(
        app,
        transfer_id,
        &mut remote_file,
        &mut local_file,
        total_bytes,
    )
    .with_context(|| format!("copy remote file to local failed: {remote_path} -> {local_path}"))?;
    emit_transfer_progress(app, transfer_id, total_bytes, total_bytes, "success");
    Ok(())
}

fn inner_sftp_download_dir(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    remote_path: &str,
    local_dir: &str,
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let total_bytes = remote_path_size(&sftp, remote_path, true)?;
    let local_root = PathBuf::from(local_dir).join(remote_name(remote_path));
    let mut transferred_bytes = 0_u64;

    fs::create_dir_all(&local_root)
        .with_context(|| format!("create local directory failed: {}", local_root.display()))?;
    emit_transfer_progress(app, transfer_id, 0, total_bytes, "running");
    download_remote_dir_recursive(
        app,
        transfer_id,
        &sftp,
        remote_path,
        &local_root,
        &mut transferred_bytes,
        total_bytes,
    )?;
    emit_transfer_progress(app, transfer_id, total_bytes, total_bytes, "success");
    Ok(())
}

fn inner_sftp_upload_file(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let mut local_file =
        File::open(local_path).with_context(|| format!("open local file failed: {local_path}"))?;
    let total_bytes = local_file
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let sftp = handle.sftp.lock();
    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .with_context(|| format!("create remote file failed: {remote_path}"))?;

    emit_transfer_progress(app, transfer_id, 0, total_bytes, "running");
    copy_with_progress(
        app,
        transfer_id,
        &mut local_file,
        &mut remote_file,
        total_bytes,
    )
    .with_context(|| format!("copy local file to remote failed: {local_path} -> {remote_path}"))?;
    emit_transfer_progress(app, transfer_id, total_bytes, total_bytes, "success");
    Ok(())
}

fn inner_sftp_upload_dir(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let local_root = PathBuf::from(local_path);
    let total_bytes = local_path_size(&local_root)?;
    let mut transferred_bytes = 0_u64;

    emit_transfer_progress(app, transfer_id, 0, total_bytes, "running");
    upload_local_path_recursive(
        app,
        transfer_id,
        &sftp,
        &local_root,
        remote_path,
        &mut transferred_bytes,
        total_bytes,
    )?;
    emit_transfer_progress(app, transfer_id, total_bytes, total_bytes, "success");
    Ok(())
}

fn copy_with_progress<R: Read, W: Write>(
    app: &AppHandle,
    transfer_id: &str,
    reader: &mut R,
    writer: &mut W,
    total_bytes: u64,
) -> Result<()> {
    let mut transferred_bytes = 0_u64;
    copy_with_progress_accum(
        app,
        transfer_id,
        reader,
        writer,
        &mut transferred_bytes,
        total_bytes,
    )
}

fn copy_with_progress_accum<R: Read, W: Write>(
    app: &AppHandle,
    transfer_id: &str,
    reader: &mut R,
    writer: &mut W,
    transferred_bytes: &mut u64,
    total_bytes: u64,
) -> Result<()> {
    let mut buffer = [0_u8; TRANSFER_BUFFER_SIZE];
    let mut last_emitted_bytes = *transferred_bytes;

    loop {
        let read_size = reader.read(&mut buffer)?;
        if read_size == 0 {
            break;
        }

        writer.write_all(&buffer[..read_size])?;
        *transferred_bytes = (*transferred_bytes).saturating_add(read_size as u64);

        if (*transferred_bytes).saturating_sub(last_emitted_bytes) >= TRANSFER_PROGRESS_STEP_BYTES
            || (total_bytes > 0 && *transferred_bytes >= total_bytes)
        {
            emit_transfer_progress(app, transfer_id, *transferred_bytes, total_bytes, "running");
            last_emitted_bytes = *transferred_bytes;
        }
    }

    writer.flush()?;
    Ok(())
}

fn emit_transfer_progress(
    app: &AppHandle,
    transfer_id: &str,
    transferred_bytes: u64,
    total_bytes: u64,
    status: &str,
) {
    let percent = if total_bytes == 0 {
        if status == "success" {
            100.0
        } else {
            0.0
        }
    } else {
        (transferred_bytes as f64 * 100.0 / total_bytes as f64).clamp(0.0, 100.0)
    };

    let _ = app.emit(
        TRANSFER_PROGRESS_EVENT,
        SftpTransferProgressPayload {
            transfer_id: transfer_id.to_string(),
            transferred_bytes,
            total_bytes,
            percent,
            status: status.to_string(),
        },
    );
}

fn inner_sftp_mkdir(state: &SftpState, connection_id: &str, path: &str) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    sftp.mkdir(Path::new(path), 0o755)
        .with_context(|| format!("create remote directory failed: {path}"))?;
    Ok(())
}

fn inner_sftp_create_file(state: &SftpState, connection_id: &str, path: &str) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let mut file = sftp
        .create(Path::new(path))
        .with_context(|| format!("create remote file failed: {path}"))?;
    file.flush()
        .with_context(|| format!("flush remote file failed: {path}"))?;
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
    delete_remote_path(&sftp, path, is_dir)
}

fn inner_sftp_delete_many(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    items: &[RemotePathInput],
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let total = items.len() as u64;

    emit_transfer_progress(app, transfer_id, 0, total, "running");
    for (index, item) in items.iter().enumerate() {
        if item.path == "/" {
            return Err(anyhow!("refuse to delete remote root path"));
        }
        delete_remote_path(&sftp, &item.path, item.is_dir)
            .with_context(|| format!("delete remote path failed: {}", item.path))?;
        emit_transfer_progress(app, transfer_id, (index + 1) as u64, total, "running");
    }
    emit_transfer_progress(app, transfer_id, total, total, "success");
    Ok(())
}

fn delete_remote_path(sftp: &Sftp, path: &str, is_dir: bool) -> Result<()> {
    if is_dir {
        delete_remote_dir_recursive(sftp, path)
    } else {
        sftp.unlink(Path::new(path))
            .with_context(|| format!("delete remote file failed: {path}"))
    }
}

fn delete_remote_dir_recursive(sftp: &Sftp, path: &str) -> Result<()> {
    let entries = sftp
        .readdir(Path::new(path))
        .with_context(|| format!("read remote dir before delete failed: {path}"))?;

    for (entry_path, stat) in entries {
        let name = entry_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        let child_path = join_remote_path(path, &name);
        delete_remote_path(sftp, &child_path, stat.is_dir())?;
    }

    sftp.rmdir(Path::new(path))
        .with_context(|| format!("delete remote directory failed: {path}"))
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

fn inner_sftp_paste(
    app: &AppHandle,
    state: &SftpState,
    connection_id: &str,
    items: &[RemoteFileItem],
    target_path: &str,
    mode: &str,
    transfer_id: &str,
) -> Result<()> {
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    let total = items.len() as u64;

    emit_transfer_progress(app, transfer_id, 0, total, "running");
    for (index, item) in items.iter().enumerate() {
        let destination = join_remote_path(target_path, &item.name);
        if item.path == destination {
            return Err(anyhow!("source and target are the same: {}", item.path));
        }

        match mode {
            "cut" => sftp
                .rename(Path::new(&item.path), Path::new(&destination), None)
                .with_context(|| format!("move remote path failed: {} -> {destination}", item.path))?,
            _ => copy_remote_path(&sftp, &item.path, &destination, item.is_dir)
                .with_context(|| format!("copy remote path failed: {} -> {destination}", item.path))?,
        }
        emit_transfer_progress(app, transfer_id, (index + 1) as u64, total, "running");
    }
    emit_transfer_progress(app, transfer_id, total, total, "success");
    Ok(())
}

fn copy_remote_path(sftp: &Sftp, source: &str, target: &str, is_dir: bool) -> Result<()> {
    if is_dir {
        copy_remote_dir_recursive(sftp, source, target)
    } else {
        copy_remote_file(sftp, source, target)
    }
}

fn copy_remote_file(sftp: &Sftp, source: &str, target: &str) -> Result<()> {
    let mut source_file = sftp
        .open(Path::new(source))
        .with_context(|| format!("open remote source file failed: {source}"))?;
    let mut target_file = sftp
        .create(Path::new(target))
        .with_context(|| format!("create remote target file failed: {target}"))?;
    std::io::copy(&mut source_file, &mut target_file)
        .with_context(|| format!("copy remote file failed: {source} -> {target}"))?;
    target_file
        .flush()
        .with_context(|| format!("flush remote target file failed: {target}"))?;
    Ok(())
}

fn copy_remote_dir_recursive(sftp: &Sftp, source: &str, target: &str) -> Result<()> {
    ensure_remote_dir(sftp, target)?;
    let entries = sftp
        .readdir(Path::new(source))
        .with_context(|| format!("read remote source directory failed: {source}"))?;

    for (entry_path, stat) in entries {
        let name = entry_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        let source_child = join_remote_path(source, &name);
        let target_child = join_remote_path(target, &name);
        copy_remote_path(sftp, &source_child, &target_child, stat.is_dir())?;
    }

    Ok(())
}

fn inner_sftp_chmod(
    state: &SftpState,
    connection_id: &str,
    path: &str,
    mode: &str,
) -> Result<()> {
    let permissions = u32::from_str_radix(mode.trim(), 8)
        .with_context(|| format!("invalid chmod mode: {mode}"))?;
    let handle = get_sftp_handle(state, connection_id)?;
    let sftp = handle.sftp.lock();
    sftp.setstat(
        Path::new(path),
        FileStat {
            size: None,
            uid: None,
            gid: None,
            perm: Some(permissions),
            atime: None,
            mtime: None,
        },
    )
    .with_context(|| format!("chmod remote path failed: {path}"))?;
    Ok(())
}

fn remote_path_size(sftp: &Sftp, path: &str, is_dir: bool) -> Result<u64> {
    if !is_dir {
        return Ok(sftp
            .stat(Path::new(path))
            .ok()
            .and_then(|stat| stat.size)
            .unwrap_or(0));
    }

    let mut total = 0_u64;
    for (entry_path, stat) in sftp
        .readdir(Path::new(path))
        .with_context(|| format!("read remote dir size failed: {path}"))?
    {
        let name = entry_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        let child_path = join_remote_path(path, &name);
        total = total.saturating_add(remote_path_size(sftp, &child_path, stat.is_dir())?);
    }
    Ok(total)
}

fn local_path_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("read local metadata failed: {}", path.display()))?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0_u64;
    for entry in fs::read_dir(path)
        .with_context(|| format!("read local directory failed: {}", path.display()))?
    {
        let entry = entry?;
        total = total.saturating_add(local_path_size(&entry.path())?);
    }
    Ok(total)
}

fn download_remote_dir_recursive(
    app: &AppHandle,
    transfer_id: &str,
    sftp: &Sftp,
    remote_path: &str,
    local_path: &Path,
    transferred_bytes: &mut u64,
    total_bytes: u64,
) -> Result<()> {
    fs::create_dir_all(local_path)
        .with_context(|| format!("create local directory failed: {}", local_path.display()))?;

    let entries = sftp
        .readdir(Path::new(remote_path))
        .with_context(|| format!("read remote dir failed: {remote_path}"))?;

    for (entry_path, stat) in entries {
        let name = entry_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        let child_remote_path = join_remote_path(remote_path, &name);
        let child_local_path = local_path.join(&name);
        if stat.is_dir() {
            download_remote_dir_recursive(
                app,
                transfer_id,
                sftp,
                &child_remote_path,
                &child_local_path,
                transferred_bytes,
                total_bytes,
            )?;
        } else {
            let mut remote_file = sftp
                .open(Path::new(&child_remote_path))
                .with_context(|| format!("open remote file failed: {child_remote_path}"))?;
            let mut local_file = File::create(&child_local_path).with_context(|| {
                format!("create local file failed: {}", child_local_path.display())
            })?;
            copy_with_progress_accum(
                app,
                transfer_id,
                &mut remote_file,
                &mut local_file,
                transferred_bytes,
                total_bytes,
            )?;
        }
    }

    Ok(())
}

fn upload_local_path_recursive(
    app: &AppHandle,
    transfer_id: &str,
    sftp: &Sftp,
    local_path: &Path,
    remote_path: &str,
    transferred_bytes: &mut u64,
    total_bytes: u64,
) -> Result<()> {
    let metadata = fs::metadata(local_path)
        .with_context(|| format!("read local metadata failed: {}", local_path.display()))?;

    if metadata.is_file() {
        let mut local_file = File::open(local_path)
            .with_context(|| format!("open local file failed: {}", local_path.display()))?;
        let mut remote_file = sftp
            .create(Path::new(remote_path))
            .with_context(|| format!("create remote file failed: {remote_path}"))?;
        copy_with_progress_accum(
            app,
            transfer_id,
            &mut local_file,
            &mut remote_file,
            transferred_bytes,
            total_bytes,
        )?;
        return Ok(());
    }

    ensure_remote_dir(sftp, remote_path)?;
    for entry in fs::read_dir(local_path)
        .with_context(|| format!("read local directory failed: {}", local_path.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let child_local_path = entry.path();
        let child_remote_path = join_remote_path(remote_path, &name);
        upload_local_path_recursive(
            app,
            transfer_id,
            sftp,
            &child_local_path,
            &child_remote_path,
            transferred_bytes,
            total_bytes,
        )?;
    }

    Ok(())
}

fn ensure_remote_dir(sftp: &Sftp, path: &str) -> Result<()> {
    if sftp.stat(Path::new(path)).is_ok_and(|stat| stat.is_dir()) {
        return Ok(());
    }

    sftp.mkdir(Path::new(path), 0o755)
        .with_context(|| format!("create remote directory failed: {path}"))?;
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

fn remote_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn format_permissions(perm: Option<u32>) -> String {
    perm.map(|value| format!("{:o}", value & 0o7777))
        .unwrap_or_else(|| "-".to_string())
}
