use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    fs,
    sync::{Mutex as AsyncMutex, OnceCell},
};

use crate::{
    atomic_file::atomic_write,
    sftp::{ConflictStrategy, SftpTransferManager},
    sftp_directory::{
        finish_persisted_replacement, prepare_local_directory, prepare_remote_directory,
        validate_persisted_replacement, DirectoryConflictStrategy, DirectoryReplacement,
        DirectoryReplacementManager,
    },
    sftp_queue::{
        cancel_directory_batch_tasks, enqueue_directory_batch, ensure_queue_capacity,
        ensure_queue_ready, pause_directory_batch_tasks, resume_directory_batch_tasks,
        retry_directory_batch_tasks, tasks_for_batch, wake_queue, EnqueueTransferRequest,
        QueueDirection, QueueTaskState, SftpTransferQueue, TransferQueueTask,
    },
    ssh::{matching_session_id, open_sftp, session_server_id, CommandError, SessionManager},
};

const BATCH_STORE_VERSION: u8 = 1;
pub(crate) const MAX_DIRECTORY_BATCH_FILES: usize = 5_000;
const MAX_DIRECTORY_BATCH_DIRECTORIES: usize = 100_000;
static BATCH_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static BATCH_CLOCK: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryBatchState {
    Preparing,
    Queued,
    Running,
    Paused,
    Committing,
    Completed,
    Failed,
    Cancelled,
    RollbackRequired,
}

impl DirectoryBatchState {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::RollbackRequired
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryCommitPhase {
    Prepared,
    Committing,
    CleanupPending,
    Completed,
    RollbackPending,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryBatch {
    version: u8,
    batch_id: String,
    name: String,
    direction: QueueDirection,
    server_id: String,
    session_id: Option<String>,
    server_label: String,
    source_directory: String,
    target_directory: String,
    write_directory: String,
    conflict_strategy: DirectoryConflictStrategy,
    replacement_id: Option<String>,
    replacement: Option<DirectoryReplacement>,
    staging_path: Option<String>,
    backup_path: Option<String>,
    task_ids: Vec<String>,
    file_count: usize,
    completed_count: usize,
    failed_count: usize,
    cancelled_count: usize,
    requires_commit: bool,
    requires_rollback: bool,
    #[serde(default)]
    cleanup_on_cancel: bool,
    commit_phase: DirectoryCommitPhase,
    state: DirectoryBatchState,
    created_at: u64,
    updated_at: u64,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBatchSnapshot {
    generated_at: u64,
    max_files_per_batch: usize,
    batches: Vec<SftpDirectoryBatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDirectoryBatchRequest {
    session_id: String,
    server_label: String,
    direction: QueueDirection,
    source_directory: String,
    target_directory: String,
    conflict_strategy: DirectoryConflictStrategy,
    directories: Vec<String>,
    file_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBatchFileRequest {
    local_path: String,
    remote_path: String,
    conflict_strategy: ConflictStrategy,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryBatchStore {
    version: u8,
    batches: Vec<SftpDirectoryBatch>,
}

impl Default for DirectoryBatchStore {
    fn default() -> Self {
        Self {
            version: BATCH_STORE_VERSION,
            batches: Vec::new(),
        }
    }
}

pub struct SftpDirectoryBatchManager {
    inner: AsyncMutex<Vec<SftpDirectoryBatch>>,
    persist_lock: AsyncMutex<()>,
    initialized: OnceCell<()>,
}

impl Default for SftpDirectoryBatchManager {
    fn default() -> Self {
        Self {
            inner: AsyncMutex::new(Vec::new()),
            persist_lock: AsyncMutex::new(()),
            initialized: OnceCell::new(),
        }
    }
}

#[tauri::command]
pub async fn sftp_batch_list(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
) -> Result<DirectoryBatchSnapshot, CommandError> {
    ensure_initialized(&app, &batches).await?;
    Ok(DirectoryBatchSnapshot {
        generated_at: unix_now(),
        max_files_per_batch: MAX_DIRECTORY_BATCH_FILES,
        batches: batches.inner.lock().await.clone(),
    })
}

#[tauri::command]
pub async fn sftp_batch_create(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    sessions: State<'_, SessionManager>,
    replacements: State<'_, DirectoryReplacementManager>,
    request: CreateDirectoryBatchRequest,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    validate_create_request(&request)?;
    ensure_queue_capacity(&app, &queue, request.file_count).await?;
    let server_id = session_server_id(&sessions, &request.session_id).await?;
    let batch_id = new_id("directory-batch");
    let replacement_id = (request.conflict_strategy == DirectoryConflictStrategy::Replace)
        .then(|| new_id("directory-replacement"));
    let now = unix_now();
    let mut batch = SftpDirectoryBatch {
        version: BATCH_STORE_VERSION,
        batch_id: batch_id.clone(),
        name: display_name(&request.source_directory),
        direction: request.direction,
        server_id: server_id.clone(),
        session_id: Some(request.session_id.clone()),
        server_label: request.server_label.trim().to_owned(),
        source_directory: request.source_directory.trim().to_owned(),
        target_directory: request.target_directory.trim().to_owned(),
        write_directory: request.target_directory.trim().to_owned(),
        conflict_strategy: request.conflict_strategy,
        replacement_id: replacement_id.clone(),
        replacement: None,
        staging_path: None,
        backup_path: None,
        task_ids: Vec::new(),
        file_count: request.file_count,
        completed_count: 0,
        failed_count: 0,
        cancelled_count: 0,
        requires_commit: false,
        requires_rollback: false,
        cleanup_on_cancel: false,
        commit_phase: DirectoryCommitPhase::Prepared,
        state: DirectoryBatchState::Preparing,
        created_at: now,
        updated_at: now,
        last_error: None,
    };
    insert_batch(&app, &batches, batch.clone()).await?;
    emit_batch(&app, &batch);

    let prepared = match prepare_batch_root(
        &sessions,
        &replacements,
        &request,
        &server_id,
        replacement_id.as_deref(),
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            let failed = mark_batch_error(
                &app,
                &batches,
                &batch_id,
                DirectoryBatchState::RollbackRequired,
                true,
                error.message.clone(),
            )
            .await?;
            emit_batch(&app, &failed);
            return Err(error);
        }
    };

    batch.write_directory = prepared.path.clone();
    batch.replacement_id = prepared.replacement_id.clone();
    batch.requires_commit = prepared.replacement_id.is_some();
    if let Some(replacement_id) = prepared.replacement_id.as_deref() {
        let replacement = replacements.get(replacement_id)?;
        validate_persisted_replacement(replacement_id, &replacement, &server_id)?;
        let (staging, backup) = replacement_paths(&replacement);
        batch.staging_path = Some(staging);
        batch.backup_path = Some(backup);
        batch.replacement = Some(replacement);
    }
    if prepared.skipped {
        batch.state = DirectoryBatchState::Completed;
        batch.commit_phase = DirectoryCommitPhase::Completed;
        batch.requires_commit = false;
    }
    batch.updated_at = unix_now();
    replace_batch(&app, &batches, batch.clone()).await?;
    emit_batch(&app, &batch);
    if !prepared.skipped {
        if let Err(error) = prepare_batch_children(
            &sessions,
            &replacements,
            &request,
            &server_id,
            &prepared.path,
        )
        .await
        {
            let failed = mark_batch_error(
                &app,
                &batches,
                &batch_id,
                DirectoryBatchState::RollbackRequired,
                true,
                error.message.clone(),
            )
            .await?;
            emit_batch(&app, &failed);
            return Err(error);
        }
    }
    Ok(batch)
}

#[tauri::command]
pub async fn sftp_batch_enqueue(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    sessions: State<'_, SessionManager>,
    batch_id: String,
    requests: Vec<DirectoryBatchFileRequest>,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    validate_safe_id(&batch_id, "目录批次标识")?;
    let batch = get_batch(&batches, &batch_id).await?;
    if batch.state != DirectoryBatchState::Preparing {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_INVALID_STATE",
            "只有准备中的目录批次可以批量入队",
        ));
    }
    if requests.len() > batch.file_count {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_FILE_COUNT_MISMATCH",
            "目录批次入队文件数量超过安全扫描结果",
        ));
    }
    let session_id = matching_session_id(&sessions, &batch.server_id)
        .await
        .ok_or_else(|| {
            CommandError::new(
                "DIRECTORY_BATCH_WAITING_FOR_SERVER",
                "等待对应服务器重新连接后继续目录批次",
            )
        })?;
    let mut queue_requests = Vec::with_capacity(requests.len());
    for request in requests {
        validate_batch_file_request(&batch, &request)?;
        queue_requests.push(EnqueueTransferRequest {
            session_id: session_id.clone(),
            server_label: batch.server_label.clone(),
            direction: batch.direction,
            local_path: request.local_path,
            remote_path: request.remote_path,
            conflict_strategy: request.conflict_strategy,
            allow_pause: false,
        });
    }
    let tasks = if queue_requests.is_empty() {
        Vec::new()
    } else {
        enqueue_directory_batch(&app, &queue, &sessions, &batch.batch_id, queue_requests).await?
    };
    let mut queued = batch;
    queued.file_count = tasks.len();
    queued.task_ids = tasks.iter().map(|task| task.task_id().to_owned()).collect();
    queued.session_id = Some(session_id);
    queued.state = if tasks.is_empty() {
        DirectoryBatchState::Committing
    } else {
        DirectoryBatchState::Queued
    };
    queued.updated_at = unix_now();
    replace_batch(&app, &batches, queued.clone()).await?;
    emit_batch(&app, &queued);
    wake_queue(&queue);
    if tasks.is_empty() {
        try_commit_batch(&app, &queued.batch_id).await;
    }
    Ok(queued)
}

#[tauri::command]
pub async fn sftp_batch_rollback(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    sessions: State<'_, SessionManager>,
    replacements: State<'_, DirectoryReplacementManager>,
    batch_id: String,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    rollback_batch_internal(&app, &batches, &sessions, &replacements, &batch_id).await
}

async fn rollback_batch_internal(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
    sessions: &SessionManager,
    replacements: &DirectoryReplacementManager,
    batch_id: &str,
) -> Result<SftpDirectoryBatch, CommandError> {
    let mut batch = get_batch(batches, batch_id).await?;
    if matches!(
        batch.state,
        DirectoryBatchState::Queued
            | DirectoryBatchState::Running
            | DirectoryBatchState::Paused
            | DirectoryBatchState::Committing
    ) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_ROLLBACK_BUSY",
            "请先取消目录批次中的传输任务，再执行回滚",
        ));
    }
    let Some(replacement) = batch.replacement.clone() else {
        batch.state = DirectoryBatchState::RollbackRequired;
        batch.requires_rollback = true;
        batch.last_error = Some(
            "该批次使用合并或重命名策略，无法证明删除现有内容安全，请人工检查目标目录".to_owned(),
        );
        batch.updated_at = unix_now();
        replace_batch(app, batches, batch.clone()).await?;
        emit_batch(app, &batch);
        return Ok(batch);
    };
    batch.commit_phase = DirectoryCommitPhase::RollbackPending;
    batch.updated_at = unix_now();
    replace_batch(app, batches, batch.clone()).await?;
    let session_id = matching_session_id(sessions, &batch.server_id).await;
    finish_persisted_replacement(
        sessions,
        batch.replacement_id.as_deref().ok_or_else(|| {
            CommandError::new("DIRECTORY_REPLACEMENT_NOT_FOUND", "目录替换标识不存在")
        })?,
        &replacement,
        &batch.server_id,
        session_id.as_deref(),
        false,
    )
    .await?;
    if let Some(replacement_id) = batch.replacement_id.as_deref() {
        replacements.remove(replacement_id);
    }
    batch.state = DirectoryBatchState::Cancelled;
    batch.requires_commit = false;
    batch.requires_rollback = false;
    batch.cleanup_on_cancel = false;
    batch.commit_phase = DirectoryCommitPhase::RolledBack;
    batch.last_error = None;
    batch.updated_at = unix_now();
    replace_batch(app, batches, batch.clone()).await?;
    emit_batch(app, &batch);
    Ok(batch)
}

#[tauri::command]
pub async fn sftp_batch_delete(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    batch_id: String,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &batches).await?;
    let batch = get_batch(&batches, &batch_id).await?;
    if !batch.state.is_terminal() || batch.requires_commit || batch.requires_rollback {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_DELETE_UNSAFE",
            "目录批次尚未安全结束，不能删除恢复记录",
        ));
    }
    remove_batch(&app, &batches, &batch_id).await
}

#[tauri::command]
pub async fn sftp_batch_wake(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &batches).await?;
    let ids = batches
        .inner
        .lock()
        .await
        .iter()
        .filter(|batch| !batch.state.is_terminal() || batch.state == DirectoryBatchState::Failed)
        .map(|batch| batch.batch_id.clone())
        .collect::<Vec<_>>();
    for batch_id in ids {
        reconcile_batch(&app, &batch_id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_batch_pause(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    transfers: State<'_, SftpTransferManager>,
    batch_id: String,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    let batch = get_batch(&batches, &batch_id).await?;
    if !matches!(
        batch.state,
        DirectoryBatchState::Queued | DirectoryBatchState::Running
    ) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_PAUSE_INVALID_STATE",
            "当前目录批次状态不能暂停",
        ));
    }
    ensure_queue_ready(&app, &queue).await?;
    pause_directory_batch_tasks(&app, &queue, &transfers, &batch_id).await?;
    reconcile_batch(&app, &batch_id).await;
    get_batch(&batches, &batch_id).await
}

#[tauri::command]
pub async fn sftp_batch_resume(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    batch_id: String,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    let batch = get_batch(&batches, &batch_id).await?;
    if batch.state != DirectoryBatchState::Paused {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_RESUME_INVALID_STATE",
            "只有已暂停目录批次可以继续",
        ));
    }
    ensure_queue_ready(&app, &queue).await?;
    resume_directory_batch_tasks(&app, &queue, &batch_id).await?;
    reconcile_batch(&app, &batch_id).await;
    get_batch(&batches, &batch_id).await
}

#[tauri::command]
pub async fn sftp_batch_retry(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    batch_id: String,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    let mut batch = get_batch(&batches, &batch_id).await?;
    if !matches!(
        batch.state,
        DirectoryBatchState::Failed | DirectoryBatchState::Cancelled
    ) || batch.commit_phase == DirectoryCommitPhase::RolledBack
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_RETRY_INVALID_STATE",
            "只有失败或已取消目录批次可以重试",
        ));
    }
    ensure_queue_ready(&app, &queue).await?;
    retry_directory_batch_tasks(&app, &queue, &batch_id).await?;
    batch.requires_rollback = false;
    batch.last_error = None;
    batch.state = DirectoryBatchState::Queued;
    batch.updated_at = unix_now();
    replace_batch(&app, &batches, batch.clone()).await?;
    emit_batch(&app, &batch);
    reconcile_batch(&app, &batch_id).await;
    get_batch(&batches, &batch_id).await
}

#[tauri::command]
pub async fn sftp_batch_cancel(
    app: AppHandle,
    batches: State<'_, SftpDirectoryBatchManager>,
    queue: State<'_, SftpTransferQueue>,
    transfers: State<'_, SftpTransferManager>,
    batch_id: String,
    delete_partial: bool,
) -> Result<SftpDirectoryBatch, CommandError> {
    ensure_initialized(&app, &batches).await?;
    let mut batch = get_batch(&batches, &batch_id).await?;
    if matches!(
        batch.state,
        DirectoryBatchState::Completed | DirectoryBatchState::RollbackRequired
    ) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_CANCEL_INVALID_STATE",
            "当前目录批次状态不能取消",
        ));
    }
    batch.cleanup_on_cancel = delete_partial;
    batch.updated_at = unix_now();
    replace_batch(&app, &batches, batch.clone()).await?;
    ensure_queue_ready(&app, &queue).await?;
    cancel_directory_batch_tasks(&app, &queue, &transfers, &batch_id, delete_partial).await?;
    reconcile_batch(&app, &batch_id).await;
    get_batch(&batches, &batch_id).await
}

pub async fn initialize_directory_batches(app: AppHandle) {
    let queue = app.state::<SftpTransferQueue>();
    if let Err(error) = ensure_queue_ready(&app, &queue).await {
        let _ = app.emit(
            "sftp-batch-error",
            serde_json::json!({ "code": error.code, "message": error.message }),
        );
        return;
    }
    let batches = app.state::<SftpDirectoryBatchManager>();
    if let Err(error) = ensure_initialized(&app, &batches).await {
        let _ = app.emit(
            "sftp-batch-error",
            serde_json::json!({ "code": error.code, "message": error.message }),
        );
        return;
    }
    let ids = batches
        .inner
        .lock()
        .await
        .iter()
        .map(|batch| batch.batch_id.clone())
        .collect::<Vec<_>>();
    for batch_id in ids {
        let current = get_batch(&batches, &batch_id).await.ok();
        let tasks = tasks_for_batch(&app, &queue, &batch_id)
            .await
            .unwrap_or_default();
        if current
            .as_ref()
            .is_some_and(|batch| batch.state == DirectoryBatchState::Preparing && tasks.is_empty())
        {
            if let Ok(batch) = mark_batch_error(
                &app,
                &batches,
                &batch_id,
                DirectoryBatchState::RollbackRequired,
                true,
                "应用在目录批次准备阶段退出，需要检查并回滚临时目录".to_owned(),
            )
            .await
            {
                emit_batch(&app, &batch);
            }
        } else {
            reconcile_batch(&app, &batch_id).await;
        }
    }
}

pub(crate) async fn handle_queue_task_update(app: &AppHandle, task: &TransferQueueTask) {
    if let Some(batch_id) = task.batch_id() {
        reconcile_batch(app, batch_id).await;
    }
}

async fn prepare_batch_root(
    sessions: &SessionManager,
    replacements: &DirectoryReplacementManager,
    request: &CreateDirectoryBatchRequest,
    server_id: &str,
    replacement_id: Option<&str>,
) -> Result<crate::sftp_directory::DirectoryPrepareResult, CommandError> {
    match request.direction {
        QueueDirection::Upload => {
            let sftp = open_sftp(sessions, &request.session_id).await?;
            let prepared = prepare_remote_directory(
                &sftp,
                replacements,
                server_id,
                request.target_directory.trim(),
                request.conflict_strategy,
                replacement_id,
            )
            .await?;
            sftp.close().await.ok();
            Ok(prepared)
        }
        QueueDirection::Download => {
            prepare_local_directory(
                replacements,
                request.target_directory.trim(),
                request.conflict_strategy,
                replacement_id,
            )
            .await
        }
    }
}

async fn prepare_batch_children(
    sessions: &SessionManager,
    replacements: &DirectoryReplacementManager,
    request: &CreateDirectoryBatchRequest,
    server_id: &str,
    write_directory: &str,
) -> Result<(), CommandError> {
    match request.direction {
        QueueDirection::Upload => {
            let sftp = open_sftp(sessions, &request.session_id).await?;
            for relative in sorted_directories(&request.directories) {
                let path = join_remote(write_directory, relative);
                prepare_remote_directory(
                    &sftp,
                    replacements,
                    server_id,
                    &path,
                    DirectoryConflictStrategy::Merge,
                    None,
                )
                .await?;
            }
            sftp.close().await.ok();
        }
        QueueDirection::Download => {
            for relative in sorted_directories(&request.directories) {
                let path = join_local(write_directory, relative);
                prepare_local_directory(
                    replacements,
                    &path,
                    DirectoryConflictStrategy::Merge,
                    None,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn reconcile_batch(app: &AppHandle, batch_id: &str) {
    let batches = app.state::<SftpDirectoryBatchManager>();
    if ensure_initialized(app, &batches).await.is_err() {
        return;
    }
    let Ok(mut batch) = get_batch(&batches, batch_id).await else {
        return;
    };
    if batch.state == DirectoryBatchState::Completed {
        return;
    }
    let queue = app.state::<SftpTransferQueue>();
    let Ok(tasks) = tasks_for_batch(app, &queue, batch_id).await else {
        return;
    };
    if tasks.iter().any(|task| !task_matches_batch(&batch, task)) {
        batch.state = DirectoryBatchState::RollbackRequired;
        batch.requires_rollback = true;
        batch.last_error =
            Some("检测到与目录批次身份或路径不匹配的子任务，已禁止自动提交".to_owned());
        batch.updated_at = unix_now();
        if replace_batch(app, &batches, batch.clone()).await.is_ok() {
            emit_batch(app, &batch);
        }
        return;
    }
    if !tasks.is_empty() {
        batch.task_ids = tasks.iter().map(|task| task.task_id().to_owned()).collect();
    }
    batch.session_id = matching_session_id(&app.state::<SessionManager>(), &batch.server_id).await;
    update_counts(&mut batch, &tasks);
    let next_state = derive_batch_state(&batch, &tasks);
    let should_commit = next_state == DirectoryBatchState::Committing;
    batch.state = next_state;
    batch.updated_at = unix_now();
    if matches!(
        batch.state,
        DirectoryBatchState::Failed | DirectoryBatchState::Cancelled
    ) && batch.replacement.is_some()
    {
        batch.requires_rollback = true;
        batch.commit_phase = DirectoryCommitPhase::RollbackPending;
    }
    if replace_batch(app, &batches, batch.clone()).await.is_err() {
        return;
    }
    emit_batch(app, &batch);
    if batch.state == DirectoryBatchState::Failed {
        let transfers = app.state::<SftpTransferManager>();
        let _ = pause_directory_batch_tasks(app, &queue, &transfers, &batch.batch_id).await;
    }
    if batch.state == DirectoryBatchState::Cancelled && batch.cleanup_on_cancel {
        let sessions = app.state::<SessionManager>();
        let replacements = app.state::<DirectoryReplacementManager>();
        let _ =
            rollback_batch_internal(app, &batches, &sessions, &replacements, &batch.batch_id).await;
        return;
    }
    if should_commit {
        try_commit_batch(app, batch_id).await;
    }
}

fn derive_batch_state(
    batch: &SftpDirectoryBatch,
    tasks: &[TransferQueueTask],
) -> DirectoryBatchState {
    if batch.state == DirectoryBatchState::RollbackRequired
        || (batch.state == DirectoryBatchState::Preparing && tasks.is_empty())
    {
        return batch.state;
    }
    if batch.file_count == 0
        || (!tasks.is_empty()
            && tasks
                .iter()
                .all(|task| task.state() == QueueTaskState::Completed))
    {
        return DirectoryBatchState::Committing;
    }
    if tasks
        .iter()
        .any(|task| task.state() == QueueTaskState::Failed)
    {
        return DirectoryBatchState::Failed;
    }
    if tasks
        .iter()
        .any(|task| task.state() == QueueTaskState::Cancelled)
    {
        return DirectoryBatchState::Cancelled;
    }
    if tasks.iter().any(|task| {
        matches!(
            task.state(),
            QueueTaskState::Paused | QueueTaskState::Pausing
        )
    }) {
        return DirectoryBatchState::Paused;
    }
    if tasks
        .iter()
        .any(|task| task.state() == QueueTaskState::Running)
    {
        return DirectoryBatchState::Running;
    }
    DirectoryBatchState::Queued
}

async fn try_commit_batch(app: &AppHandle, batch_id: &str) {
    let batches = app.state::<SftpDirectoryBatchManager>();
    let sessions = app.state::<SessionManager>();
    let replacements = app.state::<DirectoryReplacementManager>();
    let Ok(mut batch) = get_batch(&batches, batch_id).await else {
        return;
    };
    if batch.state != DirectoryBatchState::Committing {
        return;
    }
    batch.commit_phase = DirectoryCommitPhase::Committing;
    batch.requires_commit = batch.replacement.is_some();
    batch.updated_at = unix_now();
    if replace_batch(app, &batches, batch.clone()).await.is_err() {
        return;
    }
    emit_batch(app, &batch);

    if let Some(replacement) = batch.replacement.clone() {
        let Some(replacement_id) = batch.replacement_id.clone() else {
            return;
        };
        let session_id = matching_session_id(&sessions, &batch.server_id).await;
        if let Err(error) = finish_persisted_replacement(
            &sessions,
            &replacement_id,
            &replacement,
            &batch.server_id,
            session_id.as_deref(),
            true,
        )
        .await
        {
            batch.last_error = Some(error.message);
            batch.updated_at = unix_now();
            if error.code == "DIRECTORY_REPLACEMENT_SESSION_REQUIRED" {
                batch.state = DirectoryBatchState::Committing;
            } else {
                batch.state = DirectoryBatchState::RollbackRequired;
                batch.requires_rollback = true;
                batch.commit_phase = DirectoryCommitPhase::CleanupPending;
            }
            if replace_batch(app, &batches, batch.clone()).await.is_ok() {
                emit_batch(app, &batch);
            }
            return;
        }
        replacements.remove(&replacement_id);
    }
    batch.state = DirectoryBatchState::Completed;
    batch.requires_commit = false;
    batch.requires_rollback = false;
    batch.commit_phase = DirectoryCommitPhase::Completed;
    batch.last_error = None;
    batch.updated_at = unix_now();
    if replace_batch(app, &batches, batch.clone()).await.is_ok() {
        emit_batch(app, &batch);
    }
}

fn update_counts(batch: &mut SftpDirectoryBatch, tasks: &[TransferQueueTask]) {
    batch.completed_count = tasks
        .iter()
        .filter(|task| task.state() == QueueTaskState::Completed)
        .count();
    batch.failed_count = tasks
        .iter()
        .filter(|task| task.state() == QueueTaskState::Failed)
        .count();
    batch.cancelled_count = tasks
        .iter()
        .filter(|task| task.state() == QueueTaskState::Cancelled)
        .count();
}

fn task_matches_batch(batch: &SftpDirectoryBatch, task: &TransferQueueTask) -> bool {
    if task.batch_id() != Some(batch.batch_id.as_str())
        || task.server_id() != batch.server_id
        || task.direction() != batch.direction
    {
        return false;
    }
    match batch.direction {
        QueueDirection::Upload => {
            ensure_local_descendant(&batch.source_directory, task.source_path()).is_ok()
                && ensure_remote_descendant(&batch.write_directory, task.target_path()).is_ok()
        }
        QueueDirection::Download => {
            ensure_remote_descendant(&batch.source_directory, task.source_path()).is_ok()
                && ensure_local_descendant(&batch.write_directory, task.target_path()).is_ok()
        }
    }
}

async fn ensure_initialized(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
) -> Result<(), CommandError> {
    batches
        .initialized
        .get_or_try_init(|| async {
            let mut store = load_store(app).await?;
            let replacements = app.state::<DirectoryReplacementManager>();
            for batch in &mut store.batches {
                validate_loaded_batch(batch);
                if let (Some(replacement_id), Some(replacement)) =
                    (batch.replacement_id.as_deref(), batch.replacement.as_ref())
                {
                    if let Err(error) = validate_persisted_replacement(
                        replacement_id,
                        replacement,
                        &batch.server_id,
                    ) {
                        batch.state = DirectoryBatchState::RollbackRequired;
                        batch.requires_rollback = true;
                        batch.last_error = Some(error.message);
                    } else if !matches!(
                        batch.state,
                        DirectoryBatchState::Completed | DirectoryBatchState::Cancelled
                    ) {
                        if let Err(error) =
                            replacements.register(replacement_id, replacement.clone())
                        {
                            batch.state = DirectoryBatchState::RollbackRequired;
                            batch.requires_rollback = true;
                            batch.last_error = Some(error.message);
                        }
                    }
                }
                batch.updated_at = unix_now();
            }
            persist_store(app, &store).await?;
            *batches.inner.lock().await = store.batches;
            Ok(())
        })
        .await
        .map(|_| ())
}

fn validate_loaded_batch(batch: &mut SftpDirectoryBatch) {
    batch.version = BATCH_STORE_VERSION;
    if validate_safe_id(&batch.batch_id, "目录批次标识").is_err()
        || batch.file_count > MAX_DIRECTORY_BATCH_FILES
    {
        batch.state = DirectoryBatchState::RollbackRequired;
        batch.requires_rollback = true;
        batch.last_error = Some("目录批次持久化数据无效，已禁止自动恢复".to_owned());
    }
}

async fn load_store(app: &AppHandle) -> Result<DirectoryBatchStore, CommandError> {
    let path = batch_store_path(app)?;
    let content = match fs::read(&path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DirectoryBatchStore::default())
        }
        Err(error) => {
            return Err(CommandError::new(
                "DIRECTORY_BATCH_READ_FAILED",
                error.to_string(),
            ))
        }
    };
    let store: DirectoryBatchStore = serde_json::from_slice(&content)
        .map_err(|error| CommandError::new("DIRECTORY_BATCH_INVALID", error.to_string()))?;
    validate_store_version(store.version)?;
    Ok(store)
}

fn validate_store_version(version: u8) -> Result<(), CommandError> {
    if version > BATCH_STORE_VERSION {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_VERSION_UNSUPPORTED",
            "目录批次文件来自更高版本的 LiteShell",
        ));
    }
    Ok(())
}

async fn persist_store(app: &AppHandle, store: &DirectoryBatchStore) -> Result<(), CommandError> {
    let content = serde_json::to_vec_pretty(store)
        .map_err(|error| CommandError::new("DIRECTORY_BATCH_WRITE_FAILED", error.to_string()))?;
    atomic_write(&batch_store_path(app)?, &content)
        .await
        .map_err(|error| CommandError::new("DIRECTORY_BATCH_WRITE_FAILED", error.to_string()))
}

fn batch_store_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::new("DIRECTORY_BATCH_PATH_FAILED", error.to_string()))?;
    Ok(root.join("transfers").join("batches.json"))
}

async fn insert_batch(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
    batch: SftpDirectoryBatch,
) -> Result<(), CommandError> {
    let _persist = batches.persist_lock.lock().await;
    let mut inner = batches.inner.lock().await;
    if inner.iter().any(|existing| {
        existing.server_id == batch.server_id
            && existing.target_directory == batch.target_directory
            && (!matches!(
                existing.state,
                DirectoryBatchState::Completed | DirectoryBatchState::Cancelled
            ) || existing.requires_commit
                || existing.requires_rollback)
    }) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_TARGET_BUSY",
            "该服务器的目标目录已有未结束批次",
        ));
    }
    let mut next = inner.clone();
    next.push(batch);
    persist_store(
        app,
        &DirectoryBatchStore {
            version: BATCH_STORE_VERSION,
            batches: next.clone(),
        },
    )
    .await?;
    *inner = next;
    Ok(())
}

async fn replace_batch(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
    batch: SftpDirectoryBatch,
) -> Result<(), CommandError> {
    let _persist = batches.persist_lock.lock().await;
    let mut inner = batches.inner.lock().await;
    let index = inner
        .iter()
        .position(|existing| existing.batch_id == batch.batch_id)
        .ok_or_else(|| CommandError::new("DIRECTORY_BATCH_NOT_FOUND", "目录批次不存在"))?;
    let mut next = inner.clone();
    next[index] = batch;
    persist_store(
        app,
        &DirectoryBatchStore {
            version: BATCH_STORE_VERSION,
            batches: next.clone(),
        },
    )
    .await?;
    *inner = next;
    Ok(())
}

async fn remove_batch(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
    batch_id: &str,
) -> Result<(), CommandError> {
    let _persist = batches.persist_lock.lock().await;
    let mut inner = batches.inner.lock().await;
    let mut next = inner.clone();
    next.retain(|batch| batch.batch_id != batch_id);
    persist_store(
        app,
        &DirectoryBatchStore {
            version: BATCH_STORE_VERSION,
            batches: next.clone(),
        },
    )
    .await?;
    *inner = next;
    Ok(())
}

async fn get_batch(
    batches: &SftpDirectoryBatchManager,
    batch_id: &str,
) -> Result<SftpDirectoryBatch, CommandError> {
    validate_safe_id(batch_id, "目录批次标识")?;
    batches
        .inner
        .lock()
        .await
        .iter()
        .find(|batch| batch.batch_id == batch_id)
        .cloned()
        .ok_or_else(|| CommandError::new("DIRECTORY_BATCH_NOT_FOUND", "目录批次不存在"))
}

async fn mark_batch_error(
    app: &AppHandle,
    batches: &SftpDirectoryBatchManager,
    batch_id: &str,
    state: DirectoryBatchState,
    requires_rollback: bool,
    message: String,
) -> Result<SftpDirectoryBatch, CommandError> {
    let mut batch = get_batch(batches, batch_id).await?;
    batch.state = state;
    batch.requires_rollback = requires_rollback;
    batch.last_error = Some(message);
    batch.updated_at = unix_now();
    replace_batch(app, batches, batch.clone()).await?;
    Ok(batch)
}

fn validate_create_request(request: &CreateDirectoryBatchRequest) -> Result<(), CommandError> {
    if request.session_id.trim().is_empty()
        || request.source_directory.trim().is_empty()
        || request.target_directory.trim().is_empty()
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_INVALID",
            "会话、源目录和目标目录不能为空",
        ));
    }
    if request.file_count > MAX_DIRECTORY_BATCH_FILES {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_FILE_LIMIT",
            format!(
                "当前版本单次目录批次最多支持 {MAX_DIRECTORY_BATCH_FILES} 个文件，本次扫描到 {} 个",
                request.file_count
            ),
        ));
    }
    if request.directories.len() > MAX_DIRECTORY_BATCH_DIRECTORIES {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_DIRECTORY_LIMIT",
            "目录批次超过最大安全目录数量 100000",
        ));
    }
    match request.direction {
        QueueDirection::Upload => {
            let source = lexical_local_path(&request.source_directory)?;
            let metadata = std::fs::symlink_metadata(&source).map_err(|error| {
                CommandError::new("DIRECTORY_BATCH_SOURCE_INVALID", error.to_string())
            })?;
            if !metadata.is_dir() || is_local_link_or_reparse(&metadata) {
                return Err(CommandError::new(
                    "DIRECTORY_BATCH_SOURCE_UNSAFE",
                    "本地源目录必须是普通目录，不能是符号链接或 Windows junction",
                ));
            }
            let target = normalize_remote(&request.target_directory)?;
            if matches!(target.as_str(), "" | "/" | "." | "..") {
                return Err(CommandError::new(
                    "DIRECTORY_BATCH_TARGET_UNSAFE",
                    "目录批次不能写入远程根目录",
                ));
            }
        }
        QueueDirection::Download => {
            let source = normalize_remote(&request.source_directory)?;
            if matches!(source.as_str(), "" | "/" | "." | "..") {
                return Err(CommandError::new(
                    "DIRECTORY_BATCH_SOURCE_UNSAFE",
                    "目录批次不能递归下载远程根目录",
                ));
            }
            lexical_local_path(&request.target_directory)?;
        }
    }
    let mut unique = HashSet::with_capacity(request.directories.len());
    for directory in &request.directories {
        validate_relative_path(directory)?;
        if !unique.insert(normalize_relative(directory)) {
            return Err(CommandError::new(
                "DIRECTORY_BATCH_DUPLICATE_DIRECTORY",
                "目录扫描结果包含重复目录",
            ));
        }
    }
    Ok(())
}

fn is_local_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn validate_batch_file_request(
    batch: &SftpDirectoryBatch,
    request: &DirectoryBatchFileRequest,
) -> Result<(), CommandError> {
    if request.local_path.trim().is_empty() || request.remote_path.trim().is_empty() {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_FILE_INVALID",
            "目录批次文件路径不能为空",
        ));
    }
    match batch.direction {
        QueueDirection::Upload => {
            ensure_local_descendant(&batch.source_directory, &request.local_path)?;
            ensure_remote_descendant(&batch.write_directory, &request.remote_path)?;
        }
        QueueDirection::Download => {
            ensure_remote_descendant(&batch.source_directory, &request.remote_path)?;
            ensure_local_descendant(&batch.write_directory, &request.local_path)?;
        }
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), CommandError> {
    let path = path.trim();
    if path.is_empty()
        || path.contains('\0')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_RELATIVE_PATH_INVALID",
            "目录批次包含不安全的相对路径",
        ));
    }
    Ok(())
}

fn ensure_local_descendant(root: &str, path: &str) -> Result<(), CommandError> {
    let root = lexical_local_path(root)?;
    let path = lexical_local_path(path)?;
    if path == root || !path.starts_with(&root) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_PATH_OUTSIDE_ROOT",
            "目录批次文件路径超出已验证目录边界",
        ));
    }
    Ok(())
}

fn lexical_local_path(value: &str) -> Result<PathBuf, CommandError> {
    let path = PathBuf::from(value.trim());
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_LOCAL_PATH_INVALID",
            "目录批次本地路径必须是无父级跳转的绝对路径",
        ));
    }
    #[cfg(windows)]
    {
        Ok(PathBuf::from(
            path.to_string_lossy().replace('/', "\\").to_lowercase(),
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(path)
    }
}

fn ensure_remote_descendant(root: &str, path: &str) -> Result<(), CommandError> {
    let root = normalize_remote(root)?;
    let path = normalize_remote(path)?;
    let prefix = format!("{}/", root.trim_end_matches('/'));
    if !path.starts_with(&prefix) {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_PATH_OUTSIDE_ROOT",
            "目录批次远程文件路径超出已验证目录边界",
        ));
    }
    Ok(())
}

fn normalize_remote(value: &str) -> Result<String, CommandError> {
    let value = value.trim();
    if value.is_empty()
        || value.contains('\\')
        || value.contains('\0')
        || value.split('/').any(|part| part == "..")
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_REMOTE_PATH_INVALID",
            "目录批次远程路径无效",
        ));
    }
    let absolute = value.starts_with('/');
    let parts = value
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let joined = parts.join("/");
    Ok(if absolute {
        format!("/{joined}")
    } else {
        joined
    })
}

fn sorted_directories(directories: &[String]) -> Vec<&str> {
    let mut values = directories.iter().map(String::as_str).collect::<Vec<_>>();
    values.sort_by_key(|value| value.matches(['/', '\\']).count());
    values
}

fn join_remote(root: &str, relative: &str) -> String {
    format!(
        "{}/{}",
        root.trim_end_matches('/'),
        relative.replace('\\', "/").trim_start_matches('/')
    )
}

fn join_local(root: &str, relative: &str) -> String {
    Path::new(root)
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR))
        .to_string_lossy()
        .into_owned()
}

fn normalize_relative(value: &str) -> String {
    let value = value.replace('\\', "/");
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
}

fn replacement_paths(replacement: &DirectoryReplacement) -> (String, String) {
    match replacement {
        DirectoryReplacement::Local {
            staging, backup, ..
        } => (
            staging.to_string_lossy().into_owned(),
            backup.to_string_lossy().into_owned(),
        ),
        DirectoryReplacement::Remote {
            staging, backup, ..
        } => (staging.clone(), backup.clone()),
    }
}

fn validate_safe_id(value: &str, label: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(CommandError::new(
            "DIRECTORY_BATCH_ID_INVALID",
            format!("{label}无效"),
        ));
    }
    Ok(())
}

fn display_name(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .find(|value| !value.is_empty())
        .unwrap_or("directory")
        .to_owned()
}

fn new_id(prefix: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = BATCH_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{timestamp:x}-{counter:x}")
}

fn unix_now() -> u64 {
    let wall = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros(),
    )
    .unwrap_or(u64::MAX);
    let mut observed = BATCH_CLOCK.load(Ordering::Relaxed);
    loop {
        let next = wall.max(observed.saturating_add(1));
        match BATCH_CLOCK.compare_exchange_weak(
            observed,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return next,
            Err(current) => observed = current,
        }
    }
}

fn emit_batch(app: &AppHandle, batch: &SftpDirectoryBatch) {
    let _ = app.emit("sftp-directory-batch", batch.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(state: DirectoryBatchState) -> SftpDirectoryBatch {
        SftpDirectoryBatch {
            version: BATCH_STORE_VERSION,
            batch_id: "directory-batch-test".to_owned(),
            name: "test".to_owned(),
            direction: QueueDirection::Upload,
            server_id: "server".to_owned(),
            session_id: Some("session".to_owned()),
            server_label: "server".to_owned(),
            source_directory: "C:\\source".to_owned(),
            target_directory: "/target".to_owned(),
            write_directory: "/target".to_owned(),
            conflict_strategy: DirectoryConflictStrategy::Merge,
            replacement_id: None,
            replacement: None,
            staging_path: None,
            backup_path: None,
            task_ids: Vec::new(),
            file_count: 1,
            completed_count: 0,
            failed_count: 0,
            cancelled_count: 0,
            requires_commit: false,
            requires_rollback: false,
            cleanup_on_cancel: false,
            commit_phase: DirectoryCommitPhase::Prepared,
            state,
            created_at: 1,
            updated_at: 1,
            last_error: None,
        }
    }

    fn task(state: QueueTaskState) -> TransferQueueTask {
        serde_json::from_value(serde_json::json!({
            "version": 2,
            "taskId": "transfer-test",
            "batchId": "directory-batch-test",
            "sessionId": "session",
            "serverId": "server",
            "serverLabel": "server",
            "direction": "upload",
            "sourcePath": "C:\\source\\file.txt",
            "targetPath": "/target/file.txt",
            "fileName": "file.txt",
            "conflictStrategy": "overwrite",
            "state": state,
            "transferred": 0,
            "total": 1,
            "speedBytesPerSecond": 0,
            "resumedFrom": 0,
            "checkpointAvailable": false,
            "allowPause": false,
            "createdAt": 1,
            "updatedAt": 1
        }))
        .unwrap()
    }

    #[test]
    fn store_round_trip_and_version_guard() {
        let store = DirectoryBatchStore {
            version: BATCH_STORE_VERSION,
            batches: vec![batch(DirectoryBatchState::Queued)],
        };
        let encoded = serde_json::to_vec(&store).unwrap();
        let decoded: DirectoryBatchStore = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.version, BATCH_STORE_VERSION);
        assert_eq!(decoded.batches[0].batch_id, "directory-batch-test");
        assert_eq!(
            validate_store_version(BATCH_STORE_VERSION + 1)
                .unwrap_err()
                .code,
            "DIRECTORY_BATCH_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn completed_children_move_batch_to_committing() {
        let batch = batch(DirectoryBatchState::Running);
        assert_eq!(
            derive_batch_state(&batch, &[task(QueueTaskState::Completed)]),
            DirectoryBatchState::Committing
        );
    }

    #[test]
    fn failed_child_never_commits() {
        let batch = batch(DirectoryBatchState::Running);
        assert_eq!(
            derive_batch_state(&batch, &[task(QueueTaskState::Failed)]),
            DirectoryBatchState::Failed
        );
    }

    #[test]
    fn restarted_running_batch_becomes_paused_from_child_truth() {
        let batch = batch(DirectoryBatchState::Running);
        assert_eq!(
            derive_batch_state(&batch, &[task(QueueTaskState::Paused)]),
            DirectoryBatchState::Paused
        );
    }

    #[test]
    fn rejects_files_outside_batch_roots() {
        let batch = batch(DirectoryBatchState::Preparing);
        let request = DirectoryBatchFileRequest {
            local_path: "C:\\other\\file.txt".to_owned(),
            remote_path: "/target/file.txt".to_owned(),
            conflict_strategy: ConflictStrategy::Overwrite,
        };
        assert_eq!(
            validate_batch_file_request(&batch, &request)
                .unwrap_err()
                .code,
            "DIRECTORY_BATCH_PATH_OUTSIDE_ROOT"
        );
    }

    #[test]
    fn batch_does_not_take_over_a_child_from_another_server() {
        let batch = batch(DirectoryBatchState::Queued);
        let mut encoded = serde_json::to_value(task(QueueTaskState::Queued)).unwrap();
        encoded["serverId"] = serde_json::Value::String("server-b".to_owned());
        let child: TransferQueueTask = serde_json::from_value(encoded).unwrap();
        assert!(!task_matches_batch(&batch, &child));
    }

    #[test]
    fn batch_limit_is_lower_than_queue_limit() {
        const {
            assert!(MAX_DIRECTORY_BATCH_FILES < crate::sftp_queue::MAX_TASKS);
        }
    }

    #[test]
    fn file_limit_is_rejected_before_any_path_access() {
        let request = CreateDirectoryBatchRequest {
            session_id: "session".to_owned(),
            server_label: "server".to_owned(),
            direction: QueueDirection::Upload,
            source_directory: "Z:\\path-that-must-not-be-read".to_owned(),
            target_directory: "/target".to_owned(),
            conflict_strategy: DirectoryConflictStrategy::Merge,
            directories: Vec::new(),
            file_count: MAX_DIRECTORY_BATCH_FILES + 1,
        };
        assert_eq!(
            validate_create_request(&request).unwrap_err().code,
            "DIRECTORY_BATCH_FILE_LIMIT"
        );
    }
}
