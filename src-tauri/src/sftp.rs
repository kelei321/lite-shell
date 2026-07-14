use std::{
    collections::HashSet,
    io::SeekFrom,
    path::{Component, Path, PathBuf},
    sync::Mutex as StdMutex,
    time::{Duration, Instant, UNIX_EPOCH},
};

use russh_sftp::{
    client::{error::Error as SftpClientError, SftpSession},
    protocol::{FileType, StatusCode},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt},
    sync::{Mutex as AsyncMutex, Semaphore},
};

use crate::ssh::{matching_session_id, open_sftp, session_server_id, CommandError, SessionManager};

pub struct SftpTransferManager {
    cancelled: AsyncMutex<HashSet<String>>,
    active_targets: StdMutex<HashSet<TransferTargetKey>>,
    active_tasks: StdMutex<HashSet<String>>,
    slots: Semaphore,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TransferTargetKey {
    server_id: String,
    direction: TransferDirection,
    target_path: String,
}

impl TransferTargetKey {
    fn upload(server_id: &str, target_path: &str) -> Self {
        Self {
            server_id: server_id.to_owned(),
            direction: TransferDirection::Upload,
            target_path: normalize_remote_target(target_path),
        }
    }

    fn download(server_id: &str, target_path: &str) -> Self {
        Self {
            server_id: server_id.to_owned(),
            direction: TransferDirection::Download,
            target_path: normalize_local_target(target_path),
        }
    }
}

struct TransferTargetGuard<'a> {
    manager: &'a SftpTransferManager,
    key: Option<TransferTargetKey>,
}

impl Drop for TransferTargetGuard<'_> {
    fn drop(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };
        if let Ok(mut targets) = self.manager.active_targets.lock() {
            targets.remove(&key);
        }
    }
}

struct TransferTaskGuard<'a> {
    manager: &'a SftpTransferManager,
    task_id: Option<String>,
}

impl Drop for TransferTaskGuard<'_> {
    fn drop(&mut self) {
        let Some(task_id) = self.task_id.take() else {
            return;
        };
        if let Ok(mut tasks) = self.manager.active_tasks.lock() {
            tasks.remove(&task_id);
        }
    }
}

impl SftpTransferManager {
    fn acquire_target(
        &self,
        key: TransferTargetKey,
    ) -> Result<TransferTargetGuard<'_>, CommandError> {
        let mut targets = self.active_targets.lock().map_err(|_| {
            CommandError::new(
                "TRANSFER_TARGET_LOCK_FAILED",
                "传输目标锁不可用，请稍后重试",
            )
        })?;
        if !targets.insert(key.clone()) {
            return Err(CommandError::new(
                "TRANSFER_TARGET_BUSY",
                "该目标文件已有传输任务正在运行",
            ));
        }
        Ok(TransferTargetGuard {
            manager: self,
            key: Some(key),
        })
    }

    fn acquire_task(&self, task_id: &str) -> Result<TransferTaskGuard<'_>, CommandError> {
        let mut tasks = self.active_tasks.lock().map_err(|_| {
            CommandError::new("TRANSFER_TASK_LOCK_FAILED", "传输任务锁不可用，请稍后重试")
        })?;
        if !tasks.insert(task_id.to_owned()) {
            return Err(CommandError::new(
                "TRANSFER_TASK_BUSY",
                "该传输任务已经在运行",
            ));
        }
        Ok(TransferTaskGuard {
            manager: self,
            task_id: Some(task_id.to_owned()),
        })
    }
}

impl Default for SftpTransferManager {
    fn default() -> Self {
        Self {
            cancelled: AsyncMutex::new(HashSet::new()),
            active_targets: StdMutex::new(HashSet::new()),
            active_tasks: StdMutex::new(HashSet::new()),
            slots: Semaphore::new(3),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDirectoryManifest {
    root_name: String,
    directories: Vec<String>,
    files: Vec<LocalManifestFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalManifestFile {
    absolute_path: String,
    relative_path: String,
    size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecursiveDeleteResult {
    deleted_files: u64,
    deleted_directories: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    Overwrite,
    Skip,
    Rename,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    path: String,
    skipped: bool,
    resumed_from: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    path: String,
    entries: Vec<SftpEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpEntry {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
    modified_at: Option<u64>,
    permissions: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransferEvent {
    transfer_id: String,
    session_id: String,
    direction: &'static str,
    file_name: String,
    transferred: u64,
    total: u64,
    state: &'static str,
    message: Option<String>,
    speed_bytes_per_second: u64,
    eta_seconds: Option<u64>,
    resumed_from: u64,
}

const CHECKPOINT_VERSION: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransferCheckpoint {
    version: u8,
    task_id: String,
    session_id: String,
    server_id: String,
    direction: String,
    source_path: String,
    target_path: String,
    source_size: u64,
    source_modified_at: Option<u64>,
    #[serde(default)]
    source_fingerprint: String,
    temporary_path: String,
    transferred: u64,
    created_at: u64,
    updated_at: u64,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    available_session_id: Option<String>,
}

impl TransferCheckpoint {
    #[allow(clippy::too_many_arguments)]
    fn new(
        task_id: &str,
        session_id: &str,
        server_id: &str,
        direction: &str,
        source_path: &str,
        target_path: &str,
        source_size: u64,
        source_modified_at: Option<u64>,
        source_fingerprint: &str,
        temporary_path: &str,
    ) -> Self {
        let now = unix_now();
        Self {
            version: CHECKPOINT_VERSION,
            task_id: task_id.to_owned(),
            session_id: session_id.to_owned(),
            server_id: server_id.to_owned(),
            direction: direction.to_owned(),
            source_path: source_path.to_owned(),
            target_path: target_path.to_owned(),
            source_size,
            source_modified_at,
            source_fingerprint: source_fingerprint.to_owned(),
            temporary_path: temporary_path.to_owned(),
            transferred: 0,
            created_at: now,
            updated_at: now,
            available_session_id: None,
        }
    }
}

#[tauri::command]
pub async fn sftp_cancel_transfer(
    transfers: State<'_, SftpTransferManager>,
    transfer_id: String,
) -> Result<(), CommandError> {
    if transfer_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_TRANSFER", "传输标识不能为空"));
    }
    transfers.cancelled.lock().await.insert(transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn sftp_local_directory_manifest(
    path: String,
) -> Result<LocalDirectoryManifest, CommandError> {
    let root = std::path::PathBuf::from(path.trim());
    if !root.is_dir() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不存在",
        ));
    }
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别本地目录名称"))?
        .to_owned();
    let mut pending = vec![root.clone()];
    let mut directories = Vec::new();
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| CommandError::new("LOCAL_ENTRY_READ_FAILED", error.to_string()))?;
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(&root)
                .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            if file_type.is_dir() {
                directories.push(relative);
                pending.push(entry_path);
            } else if file_type.is_file() {
                let size = entry
                    .metadata()
                    .await
                    .map_err(|error| {
                        CommandError::new("LOCAL_FILE_READ_FAILED", error.to_string())
                    })?
                    .len();
                files.push(LocalManifestFile {
                    absolute_path: entry_path.to_string_lossy().into_owned(),
                    relative_path: relative,
                    size,
                });
            }
        }
    }
    directories.sort();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(LocalDirectoryManifest {
        root_name,
        directories,
        files,
    })
}

#[tauri::command]
pub async fn sftp_prepare_local_directory(
    path: String,
    conflict_strategy: ConflictStrategy,
) -> Result<TransferResult, CommandError> {
    if path.trim().is_empty() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不能为空",
        ));
    }
    let existing = fs::metadata(&path).await.ok();
    if let Some(metadata) = &existing {
        ensure_directory_target(
            metadata.is_dir(),
            "LOCAL_TARGET_IS_FILE",
            "目标路径已存在同名文件",
        )?;
    }
    let target_path = if existing.is_some() {
        match conflict_strategy {
            ConflictStrategy::Overwrite => path,
            ConflictStrategy::Skip => {
                return Ok(TransferResult {
                    path,
                    skipped: true,
                    resumed_from: 0,
                });
            }
            ConflictStrategy::Rename => unique_local_path(&path).await,
        }
    } else {
        path
    };
    fs::create_dir_all(&target_path)
        .await
        .map_err(|error| CommandError::new("LOCAL_DIRECTORY_CREATE_FAILED", error.to_string()))?;
    Ok(TransferResult {
        path: target_path,
        skipped: false,
        resumed_from: 0,
    })
}

#[tauri::command]
pub async fn sftp_list(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
) -> Result<DirectoryListing, CommandError> {
    let sftp = open_sftp(&manager, &session_id).await?;
    let canonical_path = sftp
        .canonicalize(if path.trim().is_empty() { "." } else { &path })
        .await
        .map_err(sftp_error("SFTP_PATH_FAILED"))?;
    let directory = sftp
        .read_dir(canonical_path.clone())
        .await
        .map_err(sftp_error("SFTP_LIST_FAILED"))?;
    let mut entries = directory
        .map(|entry| {
            let metadata = entry.metadata();
            let file_type = entry.file_type();
            SftpEntry {
                name: entry.file_name(),
                path: entry.path(),
                kind: file_kind(file_type),
                size: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs()),
                permissions: format!(
                    "{}{}",
                    if file_type.is_dir() { "d" } else { "-" },
                    metadata.permissions()
                ),
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| (entry.kind != "directory", entry.name.to_lowercase()));
    sftp.close().await.ok();
    Ok(DirectoryListing {
        path: canonical_path,
        entries,
    })
}

#[tauri::command]
pub async fn sftp_upload(
    app: AppHandle,
    manager: State<'_, SessionManager>,
    transfers: State<'_, SftpTransferManager>,
    session_id: String,
    local_path: String,
    remote_path: String,
    transfer_id: String,
    task_id: String,
    conflict_strategy: ConflictStrategy,
    resume: bool,
) -> Result<TransferResult, CommandError> {
    validate_transfer(&local_path, &remote_path, &transfer_id, &task_id)?;
    let _permit = transfers
        .slots
        .acquire()
        .await
        .map_err(|_| CommandError::new("TRANSFER_QUEUE_CLOSED", "传输队列已经关闭"))?;
    let _task_guard = transfers.acquire_task(&task_id)?;
    let server_id = session_server_id(&manager, &session_id).await?;
    let sftp = open_sftp(&manager, &session_id).await?;
    let existing_target = sftp.metadata(remote_path.clone()).await.ok();
    if let Some(metadata) = &existing_target {
        ensure_file_target(
            metadata.is_dir(),
            "SFTP_TARGET_IS_DIRECTORY",
            "目标路径已存在同名目录",
        )?;
    }
    let target_path = if existing_target.is_some() {
        match conflict_strategy {
            ConflictStrategy::Overwrite => remote_path,
            ConflictStrategy::Skip => {
                sftp.close().await.ok();
                return Ok(TransferResult {
                    path: remote_path,
                    skipped: true,
                    resumed_from: 0,
                });
            }
            ConflictStrategy::Rename => unique_remote_path(&sftp, &remote_path).await,
        }
    } else {
        remote_path
    };
    let _target_guard =
        transfers.acquire_target(TransferTargetKey::upload(&server_id, &target_path))?;
    let mut source = File::open(&local_path)
        .await
        .map_err(|error| CommandError::new("LOCAL_FILE_OPEN_FAILED", error.to_string()))?;
    let source_metadata = source
        .metadata()
        .await
        .map_err(|error| CommandError::new("LOCAL_FILE_READ_FAILED", error.to_string()))?;
    let total = source_metadata.len();
    let source_modified_at = modified_nanos(source_metadata.modified().ok());
    let source_fingerprint = source_sample_fingerprint(&mut source, total).await?;
    transfers.cancelled.lock().await.remove(&transfer_id);
    let temporary_path = format!("{target_path}.liteshell-{task_id}.part");
    if let Ok(metadata) = sftp.metadata(temporary_path.clone()).await {
        ensure_file_target(
            metadata.is_dir(),
            "SFTP_TEMPORARY_IS_DIRECTORY",
            "传输临时路径已被同名目录占用",
        )?;
    }
    let checkpoint_identity = TransferCheckpoint::new(
        &task_id,
        &session_id,
        &server_id,
        "upload",
        &normalize_local_target(&local_path),
        &normalize_remote_target(&target_path),
        total,
        source_modified_at,
        &source_fingerprint,
        &normalize_remote_target(&temporary_path),
    );
    let resumed_from = if resume {
        let saved = load_transfer_checkpoint(&app, &task_id).await?;
        let temporary_size = sftp
            .metadata(temporary_path.clone())
            .await
            .map_err(|_| CommandError::new("TRANSFER_RESUME_PART_MISSING", "续传临时文件不存在"))?
            .len();
        validate_resume_checkpoint(&saved, &checkpoint_identity, temporary_size)?
    } else {
        delete_transfer_checkpoint(&app, &task_id).await;
        0
    };
    let mut checkpoint = checkpoint_identity;
    checkpoint.transferred = resumed_from;
    persist_transfer_checkpoint(&app, &checkpoint).await?;
    let mut target = if resumed_from > 0 {
        source
            .seek(SeekFrom::Start(resumed_from))
            .await
            .map_err(|error| CommandError::new("LOCAL_FILE_SEEK_FAILED", error.to_string()))?;
        let mut file = sftp
            .open(temporary_path.clone())
            .await
            .map_err(sftp_error("SFTP_UPLOAD_OPEN_FAILED"))?;
        file.seek(SeekFrom::Start(resumed_from))
            .await
            .map_err(|error| CommandError::new("SFTP_UPLOAD_SEEK_FAILED", error.to_string()))?;
        file
    } else {
        sftp.create(temporary_path.clone())
            .await
            .map_err(sftp_error("SFTP_UPLOAD_OPEN_FAILED"))?
    };
    let result = transfer(
        &app,
        &session_id,
        &transfer_id,
        "upload",
        display_name(&local_path),
        total,
        resumed_from,
        &mut source,
        &mut target,
        &transfers,
        &mut checkpoint,
    )
    .await;
    if let Err(error) = result {
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "upload",
            &display_name(&local_path),
            0,
            total,
            resumed_from,
            &transfers,
            &sftp,
            error,
        )
        .await);
    }
    if let Err(error) = target.shutdown().await {
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "upload",
            &display_name(&local_path),
            total,
            total,
            resumed_from,
            &transfers,
            &sftp,
            CommandError::new("SFTP_UPLOAD_CLOSE_FAILED", error.to_string()),
        )
        .await);
    }
    let backup_path = format!("{target_path}.liteshell-{transfer_id}.backup");
    let has_original = sftp.metadata(target_path.clone()).await.is_ok();
    if has_original {
        sftp.remove_file(backup_path.clone()).await.ok();
        if let Err(error) = sftp.rename(target_path.clone(), backup_path.clone()).await {
            return Err(finish_transfer_failure(
                &app,
                &transfer_id,
                &session_id,
                "upload",
                &display_name(&local_path),
                total,
                total,
                resumed_from,
                &transfers,
                &sftp,
                sftp_error("SFTP_UPLOAD_BACKUP_FAILED")(error),
            )
            .await);
        }
    }
    if let Err(error) = sftp
        .rename(temporary_path.clone(), target_path.clone())
        .await
    {
        if has_original {
            sftp.rename(backup_path.clone(), target_path.clone())
                .await
                .ok();
        }
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "upload",
            &display_name(&local_path),
            total,
            total,
            resumed_from,
            &transfers,
            &sftp,
            CommandError::new("SFTP_UPLOAD_COMMIT_FAILED", error.to_string()),
        )
        .await);
    }
    if has_original {
        sftp.remove_file(backup_path).await.ok();
    }
    finish_transfer_success(
        &app,
        &transfer_id,
        &session_id,
        "upload",
        &display_name(&local_path),
        total,
        resumed_from,
        &transfers,
        &sftp,
        &task_id,
    )
    .await;
    Ok(TransferResult {
        path: target_path,
        skipped: false,
        resumed_from,
    })
}

#[tauri::command]
pub async fn sftp_download(
    app: AppHandle,
    manager: State<'_, SessionManager>,
    transfers: State<'_, SftpTransferManager>,
    session_id: String,
    remote_path: String,
    local_path: String,
    transfer_id: String,
    task_id: String,
    conflict_strategy: ConflictStrategy,
    resume: bool,
) -> Result<TransferResult, CommandError> {
    validate_transfer(&local_path, &remote_path, &transfer_id, &task_id)?;
    let _permit = transfers
        .slots
        .acquire()
        .await
        .map_err(|_| CommandError::new("TRANSFER_QUEUE_CLOSED", "传输队列已经关闭"))?;
    let _task_guard = transfers.acquire_task(&task_id)?;
    let server_id = session_server_id(&manager, &session_id).await?;
    let sftp = open_sftp(&manager, &session_id).await?;
    let existing_target = fs::metadata(&local_path).await.ok();
    if let Some(metadata) = &existing_target {
        ensure_file_target(
            metadata.is_dir(),
            "LOCAL_TARGET_IS_DIRECTORY",
            "目标路径已存在同名目录",
        )?;
    }
    let target_path = if existing_target.is_some() {
        match conflict_strategy {
            ConflictStrategy::Overwrite => local_path,
            ConflictStrategy::Skip => {
                sftp.close().await.ok();
                return Ok(TransferResult {
                    path: local_path,
                    skipped: true,
                    resumed_from: 0,
                });
            }
            ConflictStrategy::Rename => unique_local_path(&local_path).await,
        }
    } else {
        local_path
    };
    let _target_guard =
        transfers.acquire_target(TransferTargetKey::download(&server_id, &target_path))?;
    let source_metadata = sftp
        .metadata(remote_path.clone())
        .await
        .map_err(sftp_error("SFTP_DOWNLOAD_METADATA_FAILED"))?;
    ensure_file_target(
        source_metadata.is_dir(),
        "SFTP_SOURCE_IS_DIRECTORY",
        "远程源路径是目录，不能按文件下载",
    )?;
    let total = source_metadata.len();
    let source_modified_at = modified_nanos(source_metadata.modified().ok());
    let mut source = sftp
        .open(remote_path.clone())
        .await
        .map_err(sftp_error("SFTP_DOWNLOAD_OPEN_FAILED"))?;
    let source_fingerprint = source_sample_fingerprint(&mut source, total).await?;
    transfers.cancelled.lock().await.remove(&transfer_id);
    let temporary_path = format!("{target_path}.liteshell-{task_id}.part");
    if let Ok(metadata) = fs::metadata(&temporary_path).await {
        ensure_file_target(
            metadata.is_dir(),
            "LOCAL_TEMPORARY_IS_DIRECTORY",
            "传输临时路径已被同名目录占用",
        )?;
    }
    let checkpoint_identity = TransferCheckpoint::new(
        &task_id,
        &session_id,
        &server_id,
        "download",
        &normalize_remote_target(&remote_path),
        &normalize_local_target(&target_path),
        total,
        source_modified_at,
        &source_fingerprint,
        &normalize_local_target(&temporary_path),
    );
    let resumed_from = if resume {
        let saved = load_transfer_checkpoint(&app, &task_id).await?;
        let temporary_size = fs::metadata(&temporary_path)
            .await
            .map_err(|_| CommandError::new("TRANSFER_RESUME_PART_MISSING", "续传临时文件不存在"))?
            .len();
        validate_resume_checkpoint(&saved, &checkpoint_identity, temporary_size)?
    } else {
        delete_transfer_checkpoint(&app, &task_id).await;
        0
    };
    let mut checkpoint = checkpoint_identity;
    checkpoint.transferred = resumed_from;
    persist_transfer_checkpoint(&app, &checkpoint).await?;
    let mut target = if resumed_from > 0 {
        source
            .seek(SeekFrom::Start(resumed_from))
            .await
            .map_err(|error| CommandError::new("SFTP_DOWNLOAD_SEEK_FAILED", error.to_string()))?;
        let mut file = OpenOptions::new()
            .write(true)
            .open(&temporary_path)
            .await
            .map_err(|error| CommandError::new("LOCAL_FILE_OPEN_FAILED", error.to_string()))?;
        file.seek(SeekFrom::Start(resumed_from))
            .await
            .map_err(|error| CommandError::new("LOCAL_FILE_SEEK_FAILED", error.to_string()))?;
        file
    } else {
        File::create(&temporary_path)
            .await
            .map_err(|error| CommandError::new("LOCAL_FILE_CREATE_FAILED", error.to_string()))?
    };
    let result = transfer(
        &app,
        &session_id,
        &transfer_id,
        "download",
        display_name(&remote_path),
        total,
        resumed_from,
        &mut source,
        &mut target,
        &transfers,
        &mut checkpoint,
    )
    .await;
    if let Err(error) = result {
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "download",
            &display_name(&remote_path),
            0,
            total,
            resumed_from,
            &transfers,
            &sftp,
            error,
        )
        .await);
    }
    if let Err(error) = target.shutdown().await {
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "download",
            &display_name(&remote_path),
            total,
            total,
            resumed_from,
            &transfers,
            &sftp,
            CommandError::new("LOCAL_FILE_WRITE_FAILED", error.to_string()),
        )
        .await);
    }
    let backup_path = format!("{target_path}.liteshell-{transfer_id}.backup");
    let has_original = fs::metadata(&target_path).await.is_ok();
    if has_original {
        fs::remove_file(&backup_path).await.ok();
        if let Err(error) = fs::rename(&target_path, &backup_path).await {
            return Err(finish_transfer_failure(
                &app,
                &transfer_id,
                &session_id,
                "download",
                &display_name(&remote_path),
                total,
                total,
                resumed_from,
                &transfers,
                &sftp,
                CommandError::new("LOCAL_FILE_BACKUP_FAILED", error.to_string()),
            )
            .await);
        }
    }
    if let Err(error) = fs::rename(&temporary_path, &target_path).await {
        if has_original {
            fs::rename(&backup_path, &target_path).await.ok();
        }
        return Err(finish_transfer_failure(
            &app,
            &transfer_id,
            &session_id,
            "download",
            &display_name(&remote_path),
            total,
            total,
            resumed_from,
            &transfers,
            &sftp,
            CommandError::new("LOCAL_FILE_COMMIT_FAILED", error.to_string()),
        )
        .await);
    }
    if has_original {
        fs::remove_file(backup_path).await.ok();
    }
    finish_transfer_success(
        &app,
        &transfer_id,
        &session_id,
        "download",
        &display_name(&remote_path),
        total,
        resumed_from,
        &transfers,
        &sftp,
        &task_id,
    )
    .await;
    Ok(TransferResult {
        path: target_path,
        skipped: false,
        resumed_from,
    })
}

#[tauri::command]
pub async fn sftp_create_directory(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
) -> Result<(), CommandError> {
    validate_remote_mutation_path(&path)?;
    let sftp = open_sftp(&manager, &session_id).await?;
    if let Ok(metadata) = sftp.metadata(path.clone()).await {
        sftp.close().await.ok();
        return if metadata.is_dir() {
            Ok(())
        } else {
            Err(CommandError::new(
                "SFTP_TARGET_IS_FILE",
                "目标路径已存在同名文件",
            ))
        };
    }
    sftp.create_dir(path)
        .await
        .map_err(sftp_error("SFTP_CREATE_DIRECTORY_FAILED"))?;
    sftp.close().await.ok();
    Ok(())
}

#[tauri::command]
pub async fn sftp_rename(
    manager: State<'_, SessionManager>,
    session_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), CommandError> {
    validate_remote_mutation_path(&old_path)?;
    validate_remote_mutation_path(&new_path)?;
    let sftp = open_sftp(&manager, &session_id).await?;
    sftp.rename(old_path, new_path)
        .await
        .map_err(sftp_error("SFTP_RENAME_FAILED"))?;
    sftp.close().await.ok();
    Ok(())
}

#[tauri::command]
pub async fn sftp_delete(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
    is_directory: bool,
) -> Result<(), CommandError> {
    validate_remote_mutation_path(&path)?;
    let sftp = open_sftp(&manager, &session_id).await?;
    if is_directory {
        sftp.remove_dir(path)
            .await
            .map_err(sftp_error("SFTP_DELETE_DIRECTORY_FAILED"))?;
    } else {
        sftp.remove_file(path)
            .await
            .map_err(sftp_error("SFTP_DELETE_FILE_FAILED"))?;
    }
    sftp.close().await.ok();
    Ok(())
}

#[tauri::command]
pub async fn sftp_delete_recursive(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
) -> Result<RecursiveDeleteResult, CommandError> {
    validate_remote_mutation_path(&path)?;
    let sftp = open_sftp(&manager, &session_id).await?;
    let root = sftp
        .canonicalize(path)
        .await
        .map_err(sftp_error("SFTP_DELETE_PATH_FAILED"))?;
    validate_remote_mutation_path(&root)?;
    let root_prefix = format!("{}/", root.trim_end_matches('/'));
    let mut stack = vec![(root, false)];
    let mut deleted_files = 0_u64;
    let mut deleted_directories = 0_u64;

    while let Some((current, visited)) = stack.pop() {
        if visited {
            sftp.remove_dir(current)
                .await
                .map_err(sftp_error("SFTP_DELETE_DIRECTORY_FAILED"))?;
            deleted_directories += 1;
            continue;
        }

        stack.push((current.clone(), true));
        let entries = sftp
            .read_dir(current)
            .await
            .map_err(sftp_error("SFTP_DELETE_LIST_FAILED"))?;
        for entry in entries {
            let name = entry.file_name();
            if matches!(name.as_str(), "." | "..") {
                continue;
            }
            let child_path = entry.path();
            if !child_path.starts_with(&root_prefix) {
                sftp.close().await.ok();
                return Err(CommandError::new(
                    "SFTP_DELETE_UNSAFE_PATH",
                    "服务器返回了目标目录之外的路径，已停止删除",
                ));
            }
            let file_type = entry.file_type();
            if file_type.is_dir() && !file_type.is_symlink() {
                stack.push((child_path, false));
            } else {
                sftp.remove_file(child_path)
                    .await
                    .map_err(sftp_error("SFTP_DELETE_FILE_FAILED"))?;
                deleted_files += 1;
            }
        }
    }

    sftp.close().await.ok();
    Ok(RecursiveDeleteResult {
        deleted_files,
        deleted_directories,
    })
}

#[allow(clippy::too_many_arguments)]
async fn transfer<R, W>(
    app: &AppHandle,
    session_id: &str,
    transfer_id: &str,
    direction: &'static str,
    file_name: String,
    total: u64,
    resumed_from: u64,
    source: &mut R,
    target: &mut W,
    transfers: &State<'_, SftpTransferManager>,
    checkpoint: &mut TransferCheckpoint,
) -> Result<(), CommandError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut transferred = resumed_from;
    let started_at = Instant::now();
    let mut last_emit = Instant::now();
    let mut last_checkpoint = Instant::now();
    persist_transfer_checkpoint(app, checkpoint).await?;
    let mut buffer = vec![0_u8; 64 * 1024];
    emit_transfer(
        app,
        transfer_id,
        session_id,
        direction,
        &file_name,
        transferred,
        total,
        "running",
        None,
        0,
        None,
        resumed_from,
    );
    loop {
        if transfers.cancelled.lock().await.contains(transfer_id) {
            return Err(CommandError::new("TRANSFER_CANCELLED", "传输已取消"));
        }
        let read = source
            .read(&mut buffer)
            .await
            .map_err(|error| CommandError::new("TRANSFER_READ_FAILED", error.to_string()))?;
        if read == 0 {
            break;
        }
        target
            .write_all(&buffer[..read])
            .await
            .map_err(|error| CommandError::new("TRANSFER_WRITE_FAILED", error.to_string()))?;
        transferred += read as u64;
        checkpoint.transferred = transferred;
        if last_checkpoint.elapsed() >= Duration::from_secs(1) {
            persist_transfer_checkpoint(app, checkpoint).await?;
            last_checkpoint = Instant::now();
        }
        let elapsed = started_at.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            ((transferred - resumed_from) as f64 / elapsed) as u64
        } else {
            0
        };
        let eta = (speed > 0).then(|| total.saturating_sub(transferred).div_ceil(speed));
        if last_emit.elapsed() >= Duration::from_millis(200) || transferred >= total {
            emit_transfer(
                app,
                transfer_id,
                session_id,
                direction,
                &file_name,
                transferred,
                total,
                "running",
                None,
                speed,
                eta,
                resumed_from,
            );
            last_emit = Instant::now();
        }
    }
    checkpoint.transferred = transferred;
    persist_transfer_checkpoint(app, checkpoint).await?;
    Ok(())
}

fn terminal_state_for_error(error: &CommandError) -> &'static str {
    if error.code == "TRANSFER_CANCELLED" {
        "cancelled"
    } else {
        "failed"
    }
}

#[allow(clippy::too_many_arguments)]
async fn finish_transfer_failure(
    app: &AppHandle,
    transfer_id: &str,
    session_id: &str,
    direction: &'static str,
    file_name: &str,
    transferred: u64,
    total: u64,
    resumed_from: u64,
    transfers: &State<'_, SftpTransferManager>,
    sftp: &SftpSession,
    error: CommandError,
) -> CommandError {
    emit_transfer(
        app,
        transfer_id,
        session_id,
        direction,
        file_name,
        transferred,
        total,
        terminal_state_for_error(&error),
        Some(error.message.clone()),
        0,
        None,
        resumed_from,
    );
    transfers.cancelled.lock().await.remove(transfer_id);
    sftp.close().await.ok();
    error
}

#[allow(clippy::too_many_arguments)]
async fn finish_transfer_success(
    app: &AppHandle,
    transfer_id: &str,
    session_id: &str,
    direction: &'static str,
    file_name: &str,
    total: u64,
    resumed_from: u64,
    transfers: &State<'_, SftpTransferManager>,
    sftp: &SftpSession,
    task_id: &str,
) {
    delete_transfer_checkpoint(app, task_id).await;
    emit_transfer(
        app,
        transfer_id,
        session_id,
        direction,
        file_name,
        total,
        total,
        "completed",
        None,
        0,
        Some(0),
        resumed_from,
    );
    transfers.cancelled.lock().await.remove(transfer_id);
    sftp.close().await.ok();
}

#[allow(clippy::too_many_arguments)]
fn emit_transfer(
    app: &AppHandle,
    transfer_id: &str,
    session_id: &str,
    direction: &'static str,
    file_name: &str,
    transferred: u64,
    total: u64,
    state: &'static str,
    message: Option<String>,
    speed_bytes_per_second: u64,
    eta_seconds: Option<u64>,
    resumed_from: u64,
) {
    let _ = app.emit(
        "sftp-transfer",
        TransferEvent {
            transfer_id: transfer_id.to_owned(),
            session_id: session_id.to_owned(),
            direction,
            file_name: file_name.to_owned(),
            transferred,
            total,
            state,
            message,
            speed_bytes_per_second,
            eta_seconds,
            resumed_from,
        },
    );
}

async fn unique_remote_path(sftp: &SftpSession, path: &str) -> String {
    let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
    let (stem, extension) = split_file_name(name);
    for index in 1..10_000 {
        let candidate_name = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let candidate = if directory.is_empty() {
            candidate_name
        } else if directory == "/" {
            format!("/{candidate_name}")
        } else {
            format!("{directory}/{candidate_name}")
        };
        if sftp.metadata(candidate.clone()).await.is_err() {
            return candidate;
        }
    }
    format!("{path}.renamed")
}

async fn unique_local_path(path: &str) -> String {
    let original = PathBuf::from(path);
    let parent = original.parent().unwrap_or_else(|| Path::new(""));
    let stem = original
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = original.extension().and_then(|value| value.to_str());
    for index in 1..10_000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if fs::metadata(&candidate).await.is_err() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    format!("{path}.renamed")
}

fn split_file_name(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, extension),
        _ => (name, ""),
    }
}

fn normalize_remote_target(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(value),
        }
    }
    let normalized = segments.join("/");
    if absolute {
        format!("/{normalized}")
    } else if normalized.is_empty() {
        ".".to_owned()
    } else {
        normalized
    }
}

fn normalize_local_target(path: &str) -> String {
    let original = PathBuf::from(path);
    let absolute = if original.is_absolute() {
        original
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(original)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            value => normalized.push(value.as_os_str()),
        }
    }
    let value = normalized.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

const SOURCE_SAMPLE_SIZE: u64 = 64 * 1024;
const FNV_OFFSET_1: u64 = 0xcbf29ce484222325;
const FNV_OFFSET_2: u64 = 0x84222325cbf29ce4;
const FNV_PRIME_1: u64 = 0x100000001b3;
const FNV_PRIME_2: u64 = 0x9e3779b185ebca87;

async fn source_sample_fingerprint<R>(source: &mut R, total: u64) -> Result<String, CommandError>
where
    R: AsyncRead + AsyncSeek + Unpin,
{
    let mut first = FNV_OFFSET_1;
    let mut second = FNV_OFFSET_2;
    update_sample_hash(&mut first, &total.to_le_bytes(), FNV_PRIME_1);
    update_sample_hash(&mut second, &total.to_le_bytes(), FNV_PRIME_2);

    let samples = if total <= SOURCE_SAMPLE_SIZE * 2 {
        vec![(0, total)]
    } else {
        vec![
            (0, SOURCE_SAMPLE_SIZE),
            (total - SOURCE_SAMPLE_SIZE, SOURCE_SAMPLE_SIZE),
        ]
    };
    let mut buffer = vec![0_u8; SOURCE_SAMPLE_SIZE as usize];
    for (offset, length) in samples {
        source
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|error| {
                CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string())
            })?;
        update_sample_hash(&mut first, &offset.to_le_bytes(), FNV_PRIME_1);
        update_sample_hash(&mut second, &offset.to_le_bytes(), FNV_PRIME_2);
        let mut remaining = length;
        while remaining > 0 {
            let requested = remaining.min(buffer.len() as u64) as usize;
            let read = source
                .read(&mut buffer[..requested])
                .await
                .map_err(|error| {
                    CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string())
                })?;
            if read == 0 {
                return Err(CommandError::new(
                    "TRANSFER_SOURCE_FINGERPRINT_FAILED",
                    "读取源文件采样内容时提前结束",
                ));
            }
            update_sample_hash(&mut first, &buffer[..read], FNV_PRIME_1);
            update_sample_hash(&mut second, &buffer[..read], FNV_PRIME_2);
            remaining -= read as u64;
        }
    }
    source.seek(SeekFrom::Start(0)).await.map_err(|error| {
        CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string())
    })?;
    Ok(format!("{first:016x}{second:016x}"))
}

fn update_sample_hash(hash: &mut u64, bytes: &[u8], prime: u64) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(prime);
    }
}

fn modified_nanos(value: Option<std::time::SystemTime>) -> Option<u64> {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_nanos()).ok())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn validate_task_id(task_id: &str) -> Result<(), CommandError> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        return Err(CommandError::new(
            "INVALID_TRANSFER_TASK",
            "传输任务标识无效",
        ));
    }
    Ok(())
}

fn checkpoint_path(app: &AppHandle, task_id: &str) -> Result<PathBuf, CommandError> {
    validate_task_id(task_id)?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::new("TRANSFER_CHECKPOINT_PATH_FAILED", error.to_string()))?;
    Ok(root.join("transfers").join(format!("{task_id}.json")))
}

async fn persist_transfer_checkpoint(
    app: &AppHandle,
    checkpoint: &TransferCheckpoint,
) -> Result<(), CommandError> {
    let path = checkpoint_path(app, &checkpoint.task_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|error| {
            CommandError::new("TRANSFER_CHECKPOINT_WRITE_FAILED", error.to_string())
        })?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut persisted = checkpoint.clone();
    persisted.updated_at = unix_now();
    let content = serde_json::to_vec_pretty(&persisted).map_err(|error| {
        CommandError::new("TRANSFER_CHECKPOINT_WRITE_FAILED", error.to_string())
    })?;
    fs::write(&temporary, content).await.map_err(|error| {
        CommandError::new("TRANSFER_CHECKPOINT_WRITE_FAILED", error.to_string())
    })?;
    if let Err(error) = fs::rename(&temporary, &path).await {
        if fs::metadata(&path).await.is_ok() {
            fs::remove_file(&path).await.map_err(|remove_error| {
                CommandError::new("TRANSFER_CHECKPOINT_WRITE_FAILED", remove_error.to_string())
            })?;
            fs::rename(&temporary, &path)
                .await
                .map_err(|rename_error| {
                    CommandError::new("TRANSFER_CHECKPOINT_WRITE_FAILED", rename_error.to_string())
                })?;
        } else {
            return Err(CommandError::new(
                "TRANSFER_CHECKPOINT_WRITE_FAILED",
                error.to_string(),
            ));
        }
    }
    Ok(())
}

async fn load_transfer_checkpoint(
    app: &AppHandle,
    task_id: &str,
) -> Result<TransferCheckpoint, CommandError> {
    let path = checkpoint_path(app, task_id)?;
    let content = fs::read(path)
        .await
        .map_err(|_| CommandError::new("TRANSFER_RESUME_CHECKPOINT_MISSING", "续传检查点不存在"))?;
    serde_json::from_slice(&content)
        .map_err(|_| CommandError::new("TRANSFER_RESUME_CHECKPOINT_INVALID", "续传检查点已损坏"))
}

async fn delete_transfer_checkpoint(app: &AppHandle, task_id: &str) {
    if let Ok(path) = checkpoint_path(app, task_id) {
        fs::remove_file(path).await.ok();
    }
}

fn validate_resume_checkpoint(
    saved: &TransferCheckpoint,
    expected: &TransferCheckpoint,
    temporary_size: u64,
) -> Result<u64, CommandError> {
    let identity_matches = saved.version == expected.version
        && saved.task_id == expected.task_id
        && saved.server_id == expected.server_id
        && saved.direction == expected.direction
        && saved.source_path == expected.source_path
        && saved.target_path == expected.target_path
        && saved.source_size == expected.source_size
        && saved.source_modified_at == expected.source_modified_at
        && saved.source_fingerprint == expected.source_fingerprint
        && saved.temporary_path == expected.temporary_path;
    if !identity_matches {
        return Err(CommandError::new(
            "TRANSFER_RESUME_SOURCE_CHANGED",
            "源文件或传输目标已经变化，无法安全续传",
        ));
    }
    if temporary_size > expected.source_size || saved.transferred > temporary_size {
        return Err(CommandError::new(
            "TRANSFER_RESUME_CHECKPOINT_INVALID",
            "临时文件与续传检查点不一致",
        ));
    }
    Ok(temporary_size)
}

#[tauri::command]
pub async fn sftp_list_transfer_checkpoints(
    app: AppHandle,
    manager: State<'_, SessionManager>,
) -> Result<Vec<TransferCheckpoint>, CommandError> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::new("TRANSFER_CHECKPOINT_PATH_FAILED", error.to_string()))?
        .join("transfers");
    let mut directory = match fs::read_dir(&root).await {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(CommandError::new(
                "TRANSFER_CHECKPOINT_READ_FAILED",
                error.to_string(),
            ))
        }
    };
    let mut checkpoints = Vec::new();
    while let Some(entry) = directory
        .next_entry()
        .await
        .map_err(|error| CommandError::new("TRANSFER_CHECKPOINT_READ_FAILED", error.to_string()))?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(content) = fs::read(&path).await else {
            continue;
        };
        if let Ok(mut checkpoint) = serde_json::from_slice::<TransferCheckpoint>(&content) {
            if validate_task_id(&checkpoint.task_id).is_ok() {
                checkpoint.available_session_id =
                    matching_session_id(&manager, &checkpoint.server_id).await;
                checkpoints.push(checkpoint);
            }
        }
    }
    checkpoints.sort_by_key(|checkpoint| std::cmp::Reverse(checkpoint.updated_at));
    Ok(checkpoints)
}

#[tauri::command]
pub async fn sftp_delete_transfer_checkpoint(
    app: AppHandle,
    task_id: String,
) -> Result<(), CommandError> {
    validate_task_id(&task_id)?;
    delete_transfer_checkpoint(&app, &task_id).await;
    Ok(())
}

#[tauri::command]
pub async fn sftp_discard_transfer_checkpoint(
    app: AppHandle,
    manager: State<'_, SessionManager>,
    task_id: String,
    session_id: Option<String>,
) -> Result<(), CommandError> {
    let checkpoint = load_transfer_checkpoint(&app, &task_id).await?;
    validate_checkpoint_temporary_path(&checkpoint)?;
    if checkpoint.direction == "upload" {
        let session_id = session_id.ok_or_else(|| {
            CommandError::new(
                "TRANSFER_DISCARD_SESSION_REQUIRED",
                "删除远程临时文件前请重新连接对应服务器",
            )
        })?;
        let current_server_id = session_server_id(&manager, &session_id).await?;
        if current_server_id != checkpoint.server_id {
            return Err(CommandError::new(
                "TRANSFER_DISCARD_SERVER_MISMATCH",
                "当前 SSH 会话与检查点所属服务器不匹配",
            ));
        }
        let sftp = open_sftp(&manager, &session_id).await?;
        if let Err(error) = sftp.remove_file(checkpoint.temporary_path.clone()).await {
            if !matches!(
                &error,
                SftpClientError::Status(status) if status.status_code == StatusCode::NoSuchFile
            ) {
                sftp.close().await.ok();
                return Err(sftp_error("SFTP_TEMPORARY_DELETE_FAILED")(error));
            }
        }
        sftp.close().await.ok();
    } else {
        match fs::remove_file(&checkpoint.temporary_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(CommandError::new(
                    "LOCAL_TEMPORARY_DELETE_FAILED",
                    error.to_string(),
                ))
            }
        }
    }
    delete_transfer_checkpoint(&app, &task_id).await;
    Ok(())
}

fn validate_checkpoint_temporary_path(checkpoint: &TransferCheckpoint) -> Result<(), CommandError> {
    let expected = format!(
        "{}.liteshell-{}.part",
        checkpoint.target_path, checkpoint.task_id
    );
    if checkpoint.temporary_path != expected {
        return Err(CommandError::new(
            "TRANSFER_CHECKPOINT_INVALID",
            "检查点临时路径与传输目标不匹配",
        ));
    }
    Ok(())
}

fn ensure_file_target(
    is_directory: bool,
    code: &'static str,
    message: &'static str,
) -> Result<(), CommandError> {
    if is_directory {
        Err(CommandError::new(code, message))
    } else {
        Ok(())
    }
}

fn ensure_directory_target(
    is_directory: bool,
    code: &'static str,
    message: &'static str,
) -> Result<(), CommandError> {
    if is_directory {
        Ok(())
    } else {
        Err(CommandError::new(code, message))
    }
}

fn validate_transfer(
    local_path: &str,
    remote_path: &str,
    transfer_id: &str,
    task_id: &str,
) -> Result<(), CommandError> {
    validate_task_id(task_id)?;
    if local_path.trim().is_empty()
        || remote_path.trim().is_empty()
        || transfer_id.trim().is_empty()
    {
        return Err(CommandError::new(
            "INVALID_TRANSFER",
            "本地路径、远程路径和传输标识不能为空",
        ));
    }
    Ok(())
}

fn validate_remote_mutation_path(path: &str) -> Result<(), CommandError> {
    let path = path.trim();
    if path.is_empty() || matches!(path, "/" | "." | "..") {
        return Err(CommandError::new(
            "INVALID_REMOTE_PATH",
            "不能修改空路径、根目录或相对父目录",
        ));
    }
    Ok(())
}

fn display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn file_kind(file_type: FileType) -> &'static str {
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_file() {
        "file"
    } else {
        "other"
    }
}

fn sftp_error(code: &'static str) -> impl FnOnce(SftpClientError) -> CommandError {
    move |error| {
        let message = match &error {
            SftpClientError::Status(status) => match status.status_code {
                StatusCode::PermissionDenied => "权限不足，请检查当前用户及父目录权限".to_owned(),
                StatusCode::NoSuchFile => "文件或目录不存在，可能已被其他操作删除".to_owned(),
                StatusCode::Failure => {
                    "服务器拒绝了操作，目录可能非空、文件正在使用或权限不足".to_owned()
                }
                StatusCode::NoConnection | StatusCode::ConnectionLost => {
                    "SFTP 连接已经断开".to_owned()
                }
                StatusCode::OpUnsupported => "服务器不支持此 SFTP 操作".to_owned(),
                _ => error.to_string(),
            },
            SftpClientError::Timeout => "SFTP 操作超时".to_owned(),
            _ => error.to_string(),
        };
        CommandError::new(code, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_transfer_paths() {
        assert!(validate_transfer("C:\\a.txt", "/tmp/a.txt", "transfer-1", "task-1").is_ok());
        assert_eq!(
            validate_transfer("", "/tmp/a.txt", "transfer-1", "task-1")
                .unwrap_err()
                .code,
            "INVALID_TRANSFER"
        );
    }

    #[test]
    fn protects_remote_root_paths() {
        assert_eq!(
            validate_remote_mutation_path("/").unwrap_err().code,
            "INVALID_REMOTE_PATH"
        );
        assert!(validate_remote_mutation_path("/home/test").is_ok());
    }

    #[test]
    fn splits_names_for_conflict_renaming() {
        assert_eq!(split_file_name("archive.tar.gz"), ("archive.tar", "gz"));
        assert_eq!(split_file_name("README"), ("README", ""));
        assert_eq!(split_file_name(".env"), (".env", ""));
    }

    #[test]
    fn locks_the_same_transfer_target_until_guard_drops() {
        let manager = SftpTransferManager::default();
        let key = TransferTargetKey::upload("server-a", "/srv/file.txt");
        let first = manager.acquire_target(key.clone()).unwrap();
        let second = manager.acquire_target(key.clone());
        assert_eq!(
            second.err().map(|error| error.code),
            Some("TRANSFER_TARGET_BUSY")
        );
        drop(first);
        assert!(manager.acquire_target(key).is_ok());
    }

    #[test]
    fn allows_different_transfer_targets() {
        let manager = SftpTransferManager::default();
        let _first = manager
            .acquire_target(TransferTargetKey::upload("server-a", "/srv/a.txt"))
            .unwrap();
        assert!(manager
            .acquire_target(TransferTargetKey::upload("server-a", "/srv/b.txt"))
            .is_ok());
        assert!(manager
            .acquire_target(TransferTargetKey::download("server-a", "C:\\tmp\\a.txt"))
            .is_ok());
    }

    #[test]
    fn normalizes_transfer_target_paths() {
        assert_eq!(
            normalize_remote_target("/srv/./a/../file.txt"),
            "/srv/file.txt"
        );
        let first = normalize_local_target("tmp/../tmp/file.txt");
        let second = normalize_local_target("tmp/file.txt");
        assert_eq!(first, second);
    }

    #[test]
    fn rejects_incompatible_entry_types() {
        assert_eq!(
            ensure_file_target(true, "SFTP_TARGET_IS_DIRECTORY", "directory")
                .unwrap_err()
                .code,
            "SFTP_TARGET_IS_DIRECTORY"
        );
        assert_eq!(
            ensure_directory_target(false, "LOCAL_TARGET_IS_FILE", "file")
                .unwrap_err()
                .code,
            "LOCAL_TARGET_IS_FILE"
        );
        assert!(ensure_file_target(false, "unused", "unused").is_ok());
        assert!(ensure_directory_target(true, "unused", "unused").is_ok());
    }

    #[tokio::test]
    async fn refuses_to_prepare_a_directory_over_an_existing_file() {
        let target = std::env::temp_dir().join(format!(
            "liteshell-directory-conflict-{}",
            std::process::id()
        ));
        fs::write(&target, b"file").await.unwrap();
        let error = sftp_prepare_local_directory(
            target.to_string_lossy().into_owned(),
            ConflictStrategy::Overwrite,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "LOCAL_TARGET_IS_FILE");
        assert!(fs::metadata(&target).await.unwrap().is_file());
        fs::remove_file(target).await.unwrap();
    }

    #[test]
    fn maps_transfer_errors_to_one_terminal_state() {
        let cancelled = CommandError::new("TRANSFER_CANCELLED", "cancelled");
        let failed = CommandError::new("TRANSFER_WRITE_FAILED", "failed");
        assert_eq!(terminal_state_for_error(&cancelled), "cancelled");
        assert_eq!(terminal_state_for_error(&failed), "failed");
    }

    #[test]
    fn validates_safe_resume_identity() {
        let expected = TransferCheckpoint::new(
            "task-1",
            "session-a",
            "profile-a",
            "upload",
            "C:\\source.txt",
            "/tmp/target.txt",
            100,
            Some(10),
            "fingerprint-a",
            "/tmp/target.txt.liteshell-task-1.part",
        );
        let mut saved = expected.clone();
        saved.transferred = 50;
        assert_eq!(
            validate_resume_checkpoint(&saved, &expected, 75).unwrap(),
            75
        );

        let mut changed = expected.clone();
        changed.source_modified_at = Some(11);
        assert_eq!(
            validate_resume_checkpoint(&saved, &changed, 75)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_SOURCE_CHANGED"
        );
        let mut changed_content = expected.clone();
        changed_content.source_fingerprint = "fingerprint-b".to_owned();
        assert_eq!(
            validate_resume_checkpoint(&saved, &changed_content, 75)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_SOURCE_CHANGED"
        );
        assert_eq!(
            validate_resume_checkpoint(&saved, &expected, 101)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_CHECKPOINT_INVALID"
        );
    }

    #[test]
    fn locks_a_stable_task_id_until_guard_drops() {
        let manager = SftpTransferManager::default();
        let first = manager.acquire_task("task-a").unwrap();
        assert_eq!(
            manager.acquire_task("task-a").unwrap_err().code,
            "TRANSFER_TASK_BUSY"
        );
        drop(first);
        assert!(manager.acquire_task("task-a").is_ok());
    }

    #[tokio::test]
    async fn sampled_fingerprint_detects_same_size_content_changes() {
        let mut first = std::io::Cursor::new(vec![1_u8; 256 * 1024]);
        let mut second_data = vec![1_u8; 256 * 1024];
        second_data[0] = 2;
        let last_index = second_data.len() - 1;
        second_data[last_index] = 3;
        let mut second = std::io::Cursor::new(second_data);

        let first_fingerprint = source_sample_fingerprint(&mut first, 256 * 1024)
            .await
            .unwrap();
        let second_fingerprint = source_sample_fingerprint(&mut second, 256 * 1024)
            .await
            .unwrap();
        assert_ne!(first_fingerprint, second_fingerprint);
    }

    #[test]
    fn validates_transfer_task_ids() {
        assert!(validate_task_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert_eq!(
            validate_task_id("../bad").unwrap_err().code,
            "INVALID_TRANSFER_TASK"
        );
    }

    #[test]
    fn validates_checkpoint_temporary_path_binding() {
        let checkpoint = TransferCheckpoint::new(
            "task-1",
            "session-a",
            "profile-a",
            "download",
            "/remote/file.txt",
            "C:\\tmp\\file.txt",
            10,
            Some(1),
            "fingerprint-a",
            "C:\\tmp\\file.txt.liteshell-task-1.part",
        );
        assert!(validate_checkpoint_temporary_path(&checkpoint).is_ok());
        let mut invalid = checkpoint;
        invalid.temporary_path = "C:\\tmp\\unrelated.part".to_owned();
        assert_eq!(
            validate_checkpoint_temporary_path(&invalid)
                .unwrap_err()
                .code,
            "TRANSFER_CHECKPOINT_INVALID"
        );
        let mut wrong_target = checkpoint;
        wrong_target.target_path = "C:\\tmp\\other.txt".to_owned();
        assert_eq!(
            validate_checkpoint_temporary_path(&wrong_target)
                .unwrap_err()
                .code,
            "TRANSFER_CHECKPOINT_INVALID"
        );
    }

    #[tokio::test]
    async fn builds_local_directory_manifest() {
        let root = std::env::temp_dir().join(format!("liteshell-sftp-test-{}", std::process::id()));
        fs::create_dir_all(root.join("nested")).await.unwrap();
        fs::write(root.join("root.txt"), b"root").await.unwrap();
        fs::write(root.join("nested").join("child.txt"), b"child")
            .await
            .unwrap();

        let manifest = sftp_local_directory_manifest(root.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert!(manifest.directories.iter().any(|path| path == "nested"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.relative_path == "nested/child.txt"));

        fs::remove_dir_all(root).await.unwrap();
    }
}
