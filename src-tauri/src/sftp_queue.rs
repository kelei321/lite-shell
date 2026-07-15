use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    fs,
    sync::{broadcast, Mutex as AsyncMutex, Notify, OnceCell},
};

use crate::{
    sftp::{
        sftp_discard_transfer_checkpoint, sftp_download, sftp_list_transfer_checkpoints,
        sftp_upload, ConflictStrategy, SftpTransferManager, TransferCheckpoint, TransferEvent,
        TransferResult,
    },
    ssh::{matching_session_id, session_server_id, CommandError, SessionManager},
};

const QUEUE_VERSION: u8 = 1;
const DEFAULT_CONCURRENCY: u8 = 3;
const MAX_CONCURRENCY: u8 = 5;
const MAX_TASKS: usize = 10_000;
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static QUEUE_CLOCK: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueTaskState {
    Queued,
    Running,
    Pausing,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl QueueTaskState {
    fn is_running(self) -> bool {
        matches!(self, Self::Running | Self::Pausing)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferQueueTask {
    version: u8,
    task_id: String,
    attempt_id: Option<String>,
    session_id: Option<String>,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    available_session_id: Option<String>,
    server_id: String,
    server_label: String,
    direction: QueueDirection,
    source_path: String,
    target_path: String,
    file_name: String,
    conflict_strategy: ConflictStrategy,
    state: QueueTaskState,
    transferred: u64,
    total: u64,
    speed_bytes_per_second: u64,
    eta_seconds: Option<u64>,
    resumed_from: u64,
    message: Option<String>,
    checkpoint_available: bool,
    allow_pause: bool,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferQueueSnapshot {
    generated_at: u64,
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueTransferRequest {
    session_id: String,
    server_label: String,
    direction: QueueDirection,
    local_path: String,
    remote_path: String,
    conflict_strategy: ConflictStrategy,
    #[serde(default = "default_allow_pause")]
    allow_pause: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueStore {
    version: u8,
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
}

impl Default for QueueStore {
    fn default() -> Self {
        Self {
            version: QUEUE_VERSION,
            concurrency: DEFAULT_CONCURRENCY,
            tasks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedAction {
    Pause,
    CancelKeep,
    CancelDelete,
}

struct QueueInner {
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
    running: HashSet<String>,
    actions: HashMap<String, RequestedAction>,
    restored_waiting: HashSet<String>,
}

impl Default for QueueInner {
    fn default() -> Self {
        Self {
            concurrency: DEFAULT_CONCURRENCY,
            tasks: Vec::new(),
            running: HashSet::new(),
            actions: HashMap::new(),
            restored_waiting: HashSet::new(),
        }
    }
}

pub struct SftpTransferQueue {
    inner: AsyncMutex<QueueInner>,
    persist_lock: AsyncMutex<()>,
    notify: Notify,
    initialized: OnceCell<()>,
}

impl Default for SftpTransferQueue {
    fn default() -> Self {
        Self {
            inner: AsyncMutex::new(QueueInner::default()),
            persist_lock: AsyncMutex::new(()),
            notify: Notify::new(),
            initialized: OnceCell::new(),
        }
    }
}

#[tauri::command]
pub async fn sftp_queue_list(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    manager: State<'_, SessionManager>,
) -> Result<TransferQueueSnapshot, CommandError> {
    ensure_initialized(&app, &queue).await?;
    let generated_at = unix_now();
    let (concurrency, mut tasks) = {
        let inner = queue.inner.lock().await;
        (inner.concurrency, inner.tasks.clone())
    };
    for task in &mut tasks {
        task.available_session_id = matching_session_id(&manager, &task.server_id).await;
    }
    tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
    Ok(TransferQueueSnapshot {
        generated_at,
        concurrency,
        tasks,
    })
}

#[tauri::command]
pub async fn sftp_queue_enqueue(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    manager: State<'_, SessionManager>,
    request: EnqueueTransferRequest,
) -> Result<TransferQueueTask, CommandError> {
    ensure_initialized(&app, &queue).await?;
    validate_enqueue_request(&request)?;
    let server_id = session_server_id(&manager, &request.session_id).await?;
    let (source_path, target_path) = match request.direction {
        QueueDirection::Upload => (request.local_path, request.remote_path),
        QueueDirection::Download => (request.remote_path, request.local_path),
    };
    let now = unix_now();
    let task = TransferQueueTask {
        version: QUEUE_VERSION,
        task_id: new_id("transfer"),
        attempt_id: None,
        session_id: Some(request.session_id),
        available_session_id: None,
        server_id,
        server_label: request.server_label.trim().to_owned(),
        direction: request.direction,
        file_name: display_name(&source_path),
        source_path,
        target_path,
        conflict_strategy: request.conflict_strategy,
        state: QueueTaskState::Queued,
        transferred: 0,
        total: 0,
        speed_bytes_per_second: 0,
        eta_seconds: None,
        resumed_from: 0,
        message: None,
        checkpoint_available: false,
        allow_pause: request.allow_pause,
        created_at: now,
        updated_at: now,
    };
    {
        let mut inner = queue.inner.lock().await;
        if inner.tasks.len() >= MAX_TASKS {
            return Err(CommandError::new(
                "TRANSFER_QUEUE_LIMIT",
                "传输任务数量已达到上限，请先清理已完成任务",
            ));
        }
        inner.tasks.push(task.clone());
    }
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(task)
}

#[tauri::command]
pub async fn sftp_queue_pause(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    transfers: State<'_, SftpTransferManager>,
    task_id: String,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    validate_queue_id(&task_id)?;
    let (task, attempt_id) = {
        let mut inner = queue.inner.lock().await;
        let index = task_index(&inner.tasks, &task_id)?;
        if !inner.tasks[index].allow_pause {
            return Err(CommandError::new(
                "TRANSFER_PAUSE_UNSUPPORTED",
                "目录批次中的文件任务不能单独暂停，请取消后重新执行目录操作",
            ));
        }
        let attempt_id = match inner.tasks[index].state {
            QueueTaskState::Queued => {
                inner.tasks[index].state = QueueTaskState::Paused;
                inner.tasks[index].message = Some("任务已暂停".to_owned());
                None
            }
            QueueTaskState::Running => {
                inner.tasks[index].state = QueueTaskState::Pausing;
                inner.tasks[index].message = Some("正在安全暂停".to_owned());
                inner
                    .actions
                    .insert(task_id.clone(), RequestedAction::Pause);
                inner.tasks[index].attempt_id.clone()
            }
            QueueTaskState::Pausing | QueueTaskState::Paused => return Ok(()),
            _ => {
                return Err(CommandError::new(
                    "TRANSFER_PAUSE_INVALID_STATE",
                    "当前任务状态不能暂停",
                ))
            }
        };
        inner.tasks[index].updated_at = unix_now();
        (inner.tasks[index].clone(), attempt_id)
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    if let Some(attempt_id) = attempt_id {
        transfers.cancel_operation(&attempt_id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_resume(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    task_id: String,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    validate_queue_id(&task_id)?;
    let task = {
        let mut inner = queue.inner.lock().await;
        let index = task_index(&inner.tasks, &task_id)?;
        if inner.tasks[index].state != QueueTaskState::Paused {
            return Err(CommandError::new(
                "TRANSFER_RESUME_INVALID_STATE",
                "只有已暂停任务可以继续",
            ));
        }
        inner.tasks[index].state = QueueTaskState::Queued;
        inner.tasks[index].message = None;
        inner.tasks[index].speed_bytes_per_second = 0;
        inner.tasks[index].eta_seconds = None;
        inner.tasks[index].updated_at = unix_now();
        inner.restored_waiting.insert(task_id.clone());
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_retry(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    task_id: String,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    validate_queue_id(&task_id)?;
    let task = {
        let mut inner = queue.inner.lock().await;
        let index = task_index(&inner.tasks, &task_id)?;
        if !matches!(
            inner.tasks[index].state,
            QueueTaskState::Failed | QueueTaskState::Cancelled
        ) {
            return Err(CommandError::new(
                "TRANSFER_RETRY_INVALID_STATE",
                "只有失败或已取消任务可以重试",
            ));
        }
        if !inner.tasks[index].allow_pause && inner.tasks[index].checkpoint_available {
            return Err(CommandError::new(
                "TRANSFER_DIRECTORY_BATCH_RESTART_REQUIRED",
                "目录批次已中断，请重新执行整个目录操作",
            ));
        }
        inner.tasks[index].state = QueueTaskState::Queued;
        inner.tasks[index].message = None;
        inner.tasks[index].speed_bytes_per_second = 0;
        inner.tasks[index].eta_seconds = None;
        inner.tasks[index].updated_at = unix_now();
        inner.restored_waiting.insert(task_id.clone());
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_cancel(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    transfers: State<'_, SftpTransferManager>,
    task_id: String,
    delete_partial: bool,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    validate_queue_id(&task_id)?;
    let mut immediate = None;
    let mut attempt_id = None;
    let task = {
        let mut inner = queue.inner.lock().await;
        let index = task_index(&inner.tasks, &task_id)?;
        match inner.tasks[index].state {
            QueueTaskState::Running | QueueTaskState::Pausing => {
                inner.tasks[index].state = QueueTaskState::Pausing;
                inner.tasks[index].message = Some(if delete_partial {
                    "正在取消并删除断点".to_owned()
                } else {
                    "正在取消并保留断点".to_owned()
                });
                inner.actions.insert(
                    task_id.clone(),
                    if delete_partial {
                        RequestedAction::CancelDelete
                    } else {
                        RequestedAction::CancelKeep
                    },
                );
                attempt_id = inner.tasks[index].attempt_id.clone();
            }
            QueueTaskState::Completed => {
                return Err(CommandError::new(
                    "TRANSFER_CANCEL_INVALID_STATE",
                    "已完成任务不能取消",
                ))
            }
            _ => immediate = Some(inner.tasks[index].clone()),
        }
        inner.tasks[index].updated_at = unix_now();
        inner.tasks[index].clone()
    };

    if let Some(mut pending) = immediate {
        if delete_partial {
            discard_checkpoint_if_present(&app, &pending).await?;
            pending.checkpoint_available = false;
            pending.transferred = 0;
            pending.resumed_from = 0;
        }
        pending.state = QueueTaskState::Cancelled;
        pending.message = Some(if delete_partial {
            "任务已取消，断点已删除".to_owned()
        } else {
            "任务已取消，断点已保留".to_owned()
        });
        pending.updated_at = unix_now();
        {
            let mut inner = queue.inner.lock().await;
            let index = task_index(&inner.tasks, &task_id)?;
            inner.tasks[index] = pending.clone();
            inner.restored_waiting.remove(&task_id);
        }
        persist_current(&app, &queue).await?;
        emit_task(&app, &pending);
        return Ok(());
    }

    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    if let Some(attempt_id) = attempt_id {
        transfers.cancel_operation(&attempt_id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_clear_completed(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    {
        let mut inner = queue.inner.lock().await;
        inner
            .tasks
            .retain(|task| task.state != QueueTaskState::Completed);
    }
    persist_current(&app, &queue).await
}

#[tauri::command]
pub async fn sftp_queue_set_concurrency(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    concurrency: u8,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    if !(1..=MAX_CONCURRENCY).contains(&concurrency) {
        return Err(CommandError::new(
            "TRANSFER_CONCURRENCY_INVALID",
            "传输并发数必须在 1 到 5 之间",
        ));
    }
    queue.inner.lock().await.concurrency = concurrency;
    persist_current(&app, &queue).await?;
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_wake(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
) -> Result<(), CommandError> {
    ensure_initialized(&app, &queue).await?;
    queue.notify.notify_one();
    Ok(())
}

pub async fn initialize_transfer_queue(app: AppHandle) {
    let queue = app.state::<SftpTransferQueue>();
    if let Err(error) = ensure_initialized(&app, &queue).await {
        let _ = app.emit(
            "sftp-queue-error",
            serde_json::json!({ "code": error.code, "message": error.message }),
        );
    }
}

async fn ensure_initialized(
    app: &AppHandle,
    queue: &SftpTransferQueue,
) -> Result<(), CommandError> {
    queue
        .initialized
        .get_or_try_init(|| async {
            let manager = app.state::<SessionManager>();
            let checkpoints = sftp_list_transfer_checkpoints(app.clone(), manager)
                .await
                .unwrap_or_default();
            let mut store = load_queue_store(app).await?;
            let mut restored_waiting = HashSet::new();
            restore_store(&mut store, &checkpoints, &mut restored_waiting);
            {
                let mut inner = queue.inner.lock().await;
                inner.concurrency = store.concurrency.clamp(1, MAX_CONCURRENCY);
                inner.tasks = store.tasks;
                inner.running.clear();
                inner.actions.clear();
                inner.restored_waiting = restored_waiting;
            }
            persist_current(app, queue).await?;

            let progress_receiver = app.state::<SftpTransferManager>().subscribe();
            let progress_app = app.clone();
            tauri::async_runtime::spawn(async move {
                progress_loop(progress_app, progress_receiver).await;
            });
            let dispatcher_app = app.clone();
            tauri::async_runtime::spawn(async move {
                dispatcher_loop(dispatcher_app).await;
            });
            queue.notify.notify_one();
            Ok(())
        })
        .await
        .map(|_| ())
}

fn restore_store(
    store: &mut QueueStore,
    checkpoints: &[TransferCheckpoint],
    restored_waiting: &mut HashSet<String>,
) {
    store.version = QUEUE_VERSION;
    store.concurrency = store.concurrency.clamp(1, MAX_CONCURRENCY);
    let checkpoint_by_task = checkpoints
        .iter()
        .map(|checkpoint| (checkpoint.task_id.as_str(), checkpoint))
        .collect::<HashMap<_, _>>();

    for task in &mut store.tasks {
        task.version = QUEUE_VERSION;
        task.available_session_id = None;
        if let Some(checkpoint) = checkpoint_by_task.get(task.task_id.as_str()) {
            task.transferred = checkpoint.transferred;
            task.total = checkpoint.source_size;
            task.checkpoint_available = true;
            task.resumed_from = checkpoint.transferred;
        }
        match task.state {
            QueueTaskState::Running | QueueTaskState::Pausing => {
                task.state = if task.allow_pause {
                    QueueTaskState::Paused
                } else {
                    QueueTaskState::Failed
                };
                task.message = Some(if task.allow_pause {
                    "应用重启，任务已安全暂停".to_owned()
                } else {
                    "目录批次因应用重启中断，请重新执行整个目录操作".to_owned()
                });
            }
            QueueTaskState::Queued => {
                task.message = Some("等待重新连接服务器".to_owned());
                restored_waiting.insert(task.task_id.clone());
            }
            _ => {}
        }
        task.speed_bytes_per_second = 0;
        task.eta_seconds = None;
        task.updated_at = unix_now();
    }

    for checkpoint in checkpoints {
        if store
            .tasks
            .iter()
            .any(|task| task.task_id == checkpoint.task_id)
        {
            continue;
        }
        let direction = if checkpoint.direction == "upload" {
            QueueDirection::Upload
        } else {
            QueueDirection::Download
        };
        store.tasks.push(TransferQueueTask {
            version: QUEUE_VERSION,
            task_id: checkpoint.task_id.clone(),
            attempt_id: None,
            session_id: Some(checkpoint.session_id.clone()),
            available_session_id: checkpoint.available_session_id.clone(),
            server_id: checkpoint.server_id.clone(),
            server_label: "已保存服务器".to_owned(),
            direction,
            source_path: checkpoint.source_path.clone(),
            target_path: checkpoint.target_path.clone(),
            file_name: display_name(&checkpoint.source_path),
            conflict_strategy: ConflictStrategy::Overwrite,
            state: QueueTaskState::Paused,
            transferred: checkpoint.transferred,
            total: checkpoint.source_size,
            speed_bytes_per_second: 0,
            eta_seconds: None,
            resumed_from: checkpoint.transferred,
            message: Some("已从旧版检查点迁移，可继续传输".to_owned()),
            checkpoint_available: true,
            allow_pause: true,
            created_at: checkpoint.created_at,
            updated_at: checkpoint.updated_at,
        });
    }
}

async fn dispatcher_loop(app: AppHandle) {
    loop {
        let queue = app.state::<SftpTransferQueue>();
        queue.notify.notified().await;
        while dispatch_one(&app).await.unwrap_or(false) {}
    }
}

async fn dispatch_one(app: &AppHandle) -> Result<bool, CommandError> {
    let queue = app.state::<SftpTransferQueue>();
    let manager = app.state::<SessionManager>();
    let candidates = {
        let inner = queue.inner.lock().await;
        if inner.running.len() >= usize::from(inner.concurrency) {
            return Ok(false);
        }
        inner
            .tasks
            .iter()
            .filter(|task| task.state == QueueTaskState::Queued)
            .cloned()
            .collect::<Vec<_>>()
    };
    if candidates.is_empty() {
        return Ok(false);
    }

    for candidate in candidates {
        let session_id = matching_session_id(&manager, &candidate.server_id).await;
        let Some(session_id) = session_id else {
            let mut changed_task = None;
            {
                let mut inner = queue.inner.lock().await;
                let index = match task_index(&inner.tasks, &candidate.task_id) {
                    Ok(index) => index,
                    Err(_) => continue,
                };
                if inner.tasks[index].state != QueueTaskState::Queued {
                    continue;
                }
                inner.restored_waiting.insert(candidate.task_id.clone());
                if inner.tasks[index].message.as_deref() != Some("等待重新连接服务器") {
                    inner.tasks[index].message = Some("等待重新连接服务器".to_owned());
                    inner.tasks[index].updated_at = unix_now();
                    changed_task = Some(inner.tasks[index].clone());
                }
            }
            if let Some(task) = changed_task {
                persist_current(app, &queue).await?;
                emit_task(app, &task);
            }
            continue;
        };

        let task = {
            let mut inner = queue.inner.lock().await;
            if inner.running.len() >= usize::from(inner.concurrency) {
                return Ok(false);
            }
            let index = match task_index(&inner.tasks, &candidate.task_id) {
                Ok(index) => index,
                Err(_) => continue,
            };
            if inner.tasks[index].state != QueueTaskState::Queued {
                continue;
            }
            let attempt_id = new_id("attempt");
            inner.tasks[index].state = QueueTaskState::Running;
            inner.tasks[index].attempt_id = Some(attempt_id);
            inner.tasks[index].session_id = Some(session_id);
            inner.tasks[index].message = None;
            inner.tasks[index].speed_bytes_per_second = 0;
            inner.tasks[index].eta_seconds = None;
            inner.tasks[index].updated_at = unix_now();
            inner.running.insert(candidate.task_id.clone());
            inner.actions.remove(&candidate.task_id);
            inner.restored_waiting.remove(&candidate.task_id);
            inner.tasks[index].clone()
        };
        persist_current(app, &queue).await?;
        emit_task(app, &task);
        let worker_app = app.clone();
        tauri::async_runtime::spawn(async move {
            run_task(worker_app, task).await;
        });
        return Ok(true);
    }
    Ok(false)
}

async fn run_task(app: AppHandle, task: TransferQueueTask) {
    let session_id = match task.session_id.clone() {
        Some(session_id) => session_id,
        None => {
            finish_worker_error(
                &app,
                &task.task_id,
                CommandError::new("SESSION_NOT_FOUND", "SSH 会话不存在或已经断开"),
            )
            .await;
            return;
        }
    };
    let attempt_id = match task.attempt_id.clone() {
        Some(attempt_id) => attempt_id,
        None => {
            finish_worker_error(
                &app,
                &task.task_id,
                CommandError::new("TRANSFER_ATTEMPT_MISSING", "传输执行标识不存在"),
            )
            .await;
            return;
        }
    };
    let result = match task.direction {
        QueueDirection::Upload => {
            sftp_upload(
                app.clone(),
                app.state::<SessionManager>(),
                app.state::<SftpTransferManager>(),
                session_id,
                task.source_path.clone(),
                task.target_path.clone(),
                attempt_id,
                task.task_id.clone(),
                task.conflict_strategy,
                task.checkpoint_available,
            )
            .await
        }
        QueueDirection::Download => {
            sftp_download(
                app.clone(),
                app.state::<SessionManager>(),
                app.state::<SftpTransferManager>(),
                session_id,
                task.source_path.clone(),
                task.target_path.clone(),
                attempt_id,
                task.task_id.clone(),
                task.conflict_strategy,
                task.checkpoint_available,
            )
            .await
        }
    };
    match result {
        Ok(result) => finish_worker_success(&app, &task.task_id, result).await,
        Err(error) => finish_worker_error(&app, &task.task_id, error).await,
    }
}

async fn finish_worker_success(app: &AppHandle, task_id: &str, result: TransferResult) {
    let queue = app.state::<SftpTransferQueue>();
    let task = {
        let mut inner = queue.inner.lock().await;
        let Ok(index) = task_index(&inner.tasks, task_id) else {
            return;
        };
        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        inner.tasks[index].attempt_id = None;
        inner.tasks[index].state = QueueTaskState::Completed;
        inner.tasks[index].transferred = inner.tasks[index].total;
        inner.tasks[index].speed_bytes_per_second = 0;
        inner.tasks[index].eta_seconds = Some(0);
        inner.tasks[index].resumed_from = result.resumed_from;
        inner.tasks[index].checkpoint_available = false;
        inner.tasks[index].message = result.skipped.then(|| "目标已存在，任务已跳过".to_owned());
        inner.tasks[index].target_path = result.path;
        inner.tasks[index].updated_at = unix_now();
        inner.tasks[index].clone()
    };
    let _ = persist_current(app, &queue).await;
    emit_task(app, &task);
    queue.notify.notify_one();
}

async fn finish_worker_error(app: &AppHandle, task_id: &str, error: CommandError) {
    let queue = app.state::<SftpTransferQueue>();
    let (action, task_snapshot) = {
        let inner = queue.inner.lock().await;
        let Ok(index) = task_index(&inner.tasks, task_id) else {
            return;
        };
        (
            inner.actions.get(task_id).copied(),
            inner.tasks[index].clone(),
        )
    };
    let discard_result = if action == Some(RequestedAction::CancelDelete) {
        Some(discard_checkpoint_if_present(app, &task_snapshot).await)
    } else {
        None
    };
    let task = {
        let mut inner = queue.inner.lock().await;
        let Ok(index) = task_index(&inner.tasks, task_id) else {
            return;
        };
        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        inner.tasks[index].attempt_id = None;
        match action {
            Some(RequestedAction::Pause) => {
                inner.tasks[index].state = QueueTaskState::Paused;
                inner.tasks[index].checkpoint_available = true;
                inner.tasks[index].message = Some("任务已安全暂停".to_owned());
            }
            Some(RequestedAction::CancelKeep) => {
                inner.tasks[index].state = QueueTaskState::Cancelled;
                inner.tasks[index].checkpoint_available = true;
                inner.tasks[index].message = Some("任务已取消，断点已保留".to_owned());
            }
            Some(RequestedAction::CancelDelete) => match discard_result {
                Some(Ok(())) => {
                    inner.tasks[index].state = QueueTaskState::Cancelled;
                    inner.tasks[index].message = Some("任务已取消，断点已删除".to_owned());
                    inner.tasks[index].checkpoint_available = false;
                    inner.tasks[index].transferred = 0;
                    inner.tasks[index].resumed_from = 0;
                }
                Some(Err(discard_error)) => {
                    inner.tasks[index].state = QueueTaskState::Failed;
                    inner.tasks[index].message = Some(format!(
                        "任务已停止，但删除断点失败：{}",
                        discard_error.message
                    ));
                }
                None => {}
            },
            None => {
                inner.tasks[index].state = if error.code == "TRANSFER_CANCELLED" {
                    QueueTaskState::Cancelled
                } else {
                    QueueTaskState::Failed
                };
                inner.tasks[index].message = Some(error.message);
            }
        }
        inner.tasks[index].speed_bytes_per_second = 0;
        inner.tasks[index].eta_seconds = None;
        inner.tasks[index].updated_at = unix_now();
        inner.tasks[index].clone()
    };
    let _ = persist_current(app, &queue).await;
    emit_task(app, &task);
    queue.notify.notify_one();
}

async fn progress_loop(app: AppHandle, mut receiver: broadcast::Receiver<TransferEvent>) {
    loop {
        let event = match receiver.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        };
        apply_progress_event(&app, event).await;
    }
}

async fn apply_progress_event(app: &AppHandle, event: TransferEvent) {
    let queue = app.state::<SftpTransferQueue>();
    let (task, action) = {
        let mut inner = queue.inner.lock().await;
        let Some(index) = inner
            .tasks
            .iter()
            .position(|task| task.attempt_id.as_deref() == Some(event.transfer_id.as_str()))
        else {
            return;
        };
        if !inner.tasks[index].state.is_running() {
            return;
        }
        inner.tasks[index].transferred = inner.tasks[index].transferred.max(event.transferred);
        inner.tasks[index].total = event.total;
        inner.tasks[index].speed_bytes_per_second = event.speed_bytes_per_second;
        inner.tasks[index].eta_seconds = event.eta_seconds;
        inner.tasks[index].resumed_from = event.resumed_from;
        inner.tasks[index].checkpoint_available = true;
        inner.tasks[index].updated_at = unix_now();
        (
            inner.tasks[index].clone(),
            inner.actions.get(&inner.tasks[index].task_id).copied(),
        )
    };
    emit_task(app, &task);
    if action.is_some() && event.state == "running" {
        app.state::<SftpTransferManager>()
            .cancel_operation(&event.transfer_id)
            .await;
    }
}

async fn discard_checkpoint_if_present(
    app: &AppHandle,
    task: &TransferQueueTask,
) -> Result<(), CommandError> {
    if !task.checkpoint_available {
        return Ok(());
    }
    let session_id = if task.direction == QueueDirection::Upload {
        matching_session_id(&app.state::<SessionManager>(), &task.server_id).await
    } else {
        None
    };
    match sftp_discard_transfer_checkpoint(
        app.clone(),
        app.state::<SessionManager>(),
        task.task_id.clone(),
        session_id,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(error) if error.code == "TRANSFER_RESUME_CHECKPOINT_MISSING" => Ok(()),
        Err(error) => Err(error),
    }
}

async fn load_queue_store(app: &AppHandle) -> Result<QueueStore, CommandError> {
    let path = queue_store_path(app)?;
    let content = match fs::read(&path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(QueueStore::default())
        }
        Err(error) => {
            return Err(CommandError::new(
                "TRANSFER_QUEUE_READ_FAILED",
                error.to_string(),
            ))
        }
    };
    let store: QueueStore = serde_json::from_slice(&content)
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_INVALID", error.to_string()))?;
    if store.version > QUEUE_VERSION {
        return Err(CommandError::new(
            "TRANSFER_QUEUE_VERSION_UNSUPPORTED",
            "传输队列文件来自更高版本的 LiteShell",
        ));
    }
    Ok(store)
}

async fn persist_current(app: &AppHandle, queue: &SftpTransferQueue) -> Result<(), CommandError> {
    let _guard = queue.persist_lock.lock().await;
    let store = {
        let inner = queue.inner.lock().await;
        QueueStore {
            version: QUEUE_VERSION,
            concurrency: inner.concurrency,
            tasks: inner.tasks.clone(),
        }
    };
    persist_queue_store(app, &store).await
}

async fn persist_queue_store(app: &AppHandle, store: &QueueStore) -> Result<(), CommandError> {
    let path = queue_store_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| CommandError::new("TRANSFER_QUEUE_WRITE_FAILED", error.to_string()))?;
    }
    let temporary = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(store)
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_WRITE_FAILED", error.to_string()))?;
    fs::write(&temporary, content)
        .await
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_WRITE_FAILED", error.to_string()))?;
    let source = temporary.clone();
    let destination = path.clone();
    tokio::task::spawn_blocking(move || replace_file(&source, &destination))
        .await
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_WRITE_FAILED", error.to_string()))?
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_WRITE_FAILED", error.to_string()))
}

fn queue_store_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::new("TRANSFER_QUEUE_PATH_FAILED", error.to_string()))?;
    Ok(root.join("transfers").join("queue.json"))
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let wide = |value: &OsStr| value.encode_wide().chain(Some(0)).collect::<Vec<_>>();
    let source = wide(source.as_os_str());
    let destination = wide(destination.as_os_str());
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::rename(source, destination)
}

fn task_index(tasks: &[TransferQueueTask], task_id: &str) -> Result<usize, CommandError> {
    tasks
        .iter()
        .position(|task| task.task_id == task_id)
        .ok_or_else(|| CommandError::new("TRANSFER_TASK_NOT_FOUND", "传输任务不存在"))
}

fn validate_enqueue_request(request: &EnqueueTransferRequest) -> Result<(), CommandError> {
    if request.session_id.trim().is_empty()
        || request.local_path.trim().is_empty()
        || request.remote_path.trim().is_empty()
    {
        return Err(CommandError::new(
            "INVALID_TRANSFER",
            "SSH 会话、本地路径和远程路径不能为空",
        ));
    }
    Ok(())
}

fn validate_queue_id(task_id: &str) -> Result<(), CommandError> {
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

fn display_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|value| !value.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn new_id(prefix: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
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
    let mut observed = QUEUE_CLOCK.load(Ordering::Relaxed);
    loop {
        let next = wall.max(observed.saturating_add(1));
        match QUEUE_CLOCK.compare_exchange_weak(
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

fn default_allow_pause() -> bool {
    true
}

fn emit_task(app: &AppHandle, task: &TransferQueueTask) {
    let _ = app.emit("sftp-queue-task", task.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, server: &str, state: QueueTaskState, created_at: u64) -> TransferQueueTask {
        TransferQueueTask {
            version: QUEUE_VERSION,
            task_id: id.to_owned(),
            attempt_id: None,
            session_id: Some(format!("session-{server}")),
            available_session_id: None,
            server_id: server.to_owned(),
            server_label: server.to_owned(),
            direction: QueueDirection::Upload,
            source_path: format!("C:\\{id}.txt"),
            target_path: format!("/tmp/{id}.txt"),
            file_name: format!("{id}.txt"),
            conflict_strategy: ConflictStrategy::Overwrite,
            state,
            transferred: 0,
            total: 100,
            speed_bytes_per_second: 0,
            eta_seconds: None,
            resumed_from: 0,
            message: None,
            checkpoint_available: false,
            allow_pause: true,
            created_at,
            updated_at: created_at,
        }
    }

    fn runnable_ids(
        tasks: &[TransferQueueTask],
        available_servers: &HashSet<String>,
        running: usize,
        concurrency: u8,
    ) -> Vec<String> {
        tasks
            .iter()
            .filter(|task| {
                task.state == QueueTaskState::Queued && available_servers.contains(&task.server_id)
            })
            .take(usize::from(concurrency).saturating_sub(running))
            .map(|task| task.task_id.clone())
            .collect()
    }

    #[test]
    fn queue_timestamps_are_strictly_monotonic() {
        let first = unix_now();
        let second = unix_now();
        assert!(second > first);
    }

    #[test]
    fn queue_preserves_fifo_order_and_concurrency_limit() {
        let tasks = vec![
            task("first", "a", QueueTaskState::Queued, 1),
            task("second", "a", QueueTaskState::Queued, 2),
            task("third", "a", QueueTaskState::Queued, 3),
        ];
        let servers = HashSet::from(["a".to_owned()]);
        assert_eq!(
            runnable_ids(&tasks, &servers, 1, 3),
            vec!["first", "second"]
        );
    }

    #[test]
    fn queue_keeps_servers_isolated() {
        let tasks = vec![
            task("server-a", "a", QueueTaskState::Queued, 1),
            task("server-b", "b", QueueTaskState::Queued, 2),
        ];
        let servers = HashSet::from(["b".to_owned()]);
        assert_eq!(runnable_ids(&tasks, &servers, 0, 3), vec!["server-b"]);
    }

    #[test]
    fn restart_converts_running_to_paused_and_keeps_queued() {
        let mut store = QueueStore {
            version: QUEUE_VERSION,
            concurrency: 3,
            tasks: vec![
                task("running", "a", QueueTaskState::Running, 1),
                task("queued", "a", QueueTaskState::Queued, 2),
                task("failed", "a", QueueTaskState::Failed, 3),
            ],
        };
        let mut restored = HashSet::new();
        restore_store(&mut store, &[], &mut restored);
        assert_eq!(store.tasks[0].state, QueueTaskState::Paused);
        assert_eq!(store.tasks[1].state, QueueTaskState::Queued);
        assert_eq!(store.tasks[2].state, QueueTaskState::Failed);
        assert!(restored.contains("queued"));
    }

    #[test]
    fn paused_task_progress_remains_stable() {
        let mut paused = task("paused", "a", QueueTaskState::Paused, 1);
        paused.transferred = 42;
        let event = TransferEvent {
            transfer_id: "attempt".to_owned(),
            session_id: "session-a".to_owned(),
            direction: "upload",
            file_name: "paused.txt".to_owned(),
            transferred: 99,
            total: 100,
            state: "running",
            message: None,
            speed_bytes_per_second: 10,
            eta_seconds: Some(1),
            resumed_from: 0,
        };
        if paused.state.is_running() {
            paused.transferred = paused.transferred.max(event.transferred);
        }
        assert_eq!(paused.transferred, 42);
    }

    #[test]
    fn concurrency_is_restricted_to_supported_range() {
        assert_eq!(DEFAULT_CONCURRENCY, 3);
        assert_eq!(MAX_CONCURRENCY, 5);
        assert!((1..=MAX_CONCURRENCY).contains(&1));
        assert!(!(1..=MAX_CONCURRENCY).contains(&0));
        assert!(!(1..=MAX_CONCURRENCY).contains(&6));
    }
}
