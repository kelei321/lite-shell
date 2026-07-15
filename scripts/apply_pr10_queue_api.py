from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


queue_path = Path("src-tauri/src/sftp_queue.rs")
queue = queue_path.read_text(encoding="utf-8")

queue = replace_once(
    queue,
    '''#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueTransferRequest {''',
    '''#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferQueueSnapshot {
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueTransferRequest {''',
    "queue snapshot type",
)

queue = replace_once(
    queue,
    '''pub async fn sftp_queue_list(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    manager: State<'_, SessionManager>,
) -> Result<Vec<TransferQueueTask>, CommandError> {
    ensure_initialized(&app, &queue).await?;
    let mut tasks = queue.inner.lock().await.tasks.clone();
    for task in &mut tasks {
        task.available_session_id = matching_session_id(&manager, &task.server_id).await;
    }
    tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
    Ok(tasks)
}''',
    '''pub async fn sftp_queue_list(
    app: AppHandle,
    queue: State<'_, SftpTransferQueue>,
    manager: State<'_, SessionManager>,
) -> Result<TransferQueueSnapshot, CommandError> {
    ensure_initialized(&app, &queue).await?;
    let (concurrency, mut tasks) = {
        let inner = queue.inner.lock().await;
        (inner.concurrency, inner.tasks.clone())
    };
    for task in &mut tasks {
        task.available_session_id = matching_session_id(&manager, &task.server_id).await;
    }
    tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
    Ok(TransferQueueSnapshot { concurrency, tasks })
}''',
    "queue list snapshot",
)

queue_path.write_text(queue, encoding="utf-8")

service_path = Path("src/services/ssh.ts")
service = service_path.read_text(encoding="utf-8")

service = replace_once(
    service,
    '''export type ConflictStrategy = "overwrite" | "skip" | "rename";
export type DirectoryConflictStrategy = "merge" | "skip" | "rename" | "replace";''',
    '''export type ConflictStrategy = "overwrite" | "skip" | "rename";
export type TransferQueueState =
  | "queued"
  | "running"
  | "pausing"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type TransferQueueTask = {
  version: number;
  taskId: string;
  attemptId?: string;
  sessionId?: string;
  availableSessionId?: string;
  serverId: string;
  serverLabel: string;
  direction: "upload" | "download";
  sourcePath: string;
  targetPath: string;
  fileName: string;
  conflictStrategy: ConflictStrategy;
  state: TransferQueueState;
  transferred: number;
  total: number;
  speedBytesPerSecond: number;
  etaSeconds?: number | null;
  resumedFrom: number;
  message?: string;
  checkpointAvailable: boolean;
  allowPause: boolean;
  createdAt: number;
  updatedAt: number;
};

export type TransferQueueSnapshot = {
  concurrency: number;
  tasks: TransferQueueTask[];
};

export type DirectoryConflictStrategy = "merge" | "skip" | "rename" | "replace";''',
    "queue service types",
)

service = replace_once(
    service,
    '''export const discardSftpTransferCheckpoint = (taskId: string, sessionId?: string) =>
  invoke<void>("sftp_discard_transfer_checkpoint", { taskId, sessionId });

export const getLocalDirectoryManifest''',
    '''export const discardSftpTransferCheckpoint = (taskId: string, sessionId?: string) =>
  invoke<void>("sftp_discard_transfer_checkpoint", { taskId, sessionId });

export const listSftpTransferQueue = () =>
  invoke<TransferQueueSnapshot>("sftp_queue_list");

export const enqueueSftpTransfer = (request: {
  sessionId: string;
  serverLabel: string;
  direction: "upload" | "download";
  localPath: string;
  remotePath: string;
  conflictStrategy: ConflictStrategy;
  allowPause?: boolean;
}) => invoke<TransferQueueTask>("sftp_queue_enqueue", { request });

export const pauseSftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_pause", { taskId });

export const resumeSftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_resume", { taskId });

export const retrySftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_retry", { taskId });

export const cancelQueuedSftpTransfer = (taskId: string, deletePartial: boolean) =>
  invoke<void>("sftp_queue_cancel", { taskId, deletePartial });

export const clearCompletedSftpTransfers = () =>
  invoke<void>("sftp_queue_clear_completed");

export const setSftpTransferConcurrency = (concurrency: number) =>
  invoke<void>("sftp_queue_set_concurrency", { concurrency });

export const wakeSftpTransferQueue = () =>
  invoke<void>("sftp_queue_wake");

export const getLocalDirectoryManifest''',
    "queue service commands",
)

service = replace_once(
    service,
    '''export const listenSftpTransfers = (handler: (event: TransferEvent) => void): Promise<UnlistenFn> =>
  listen<TransferEvent>("sftp-transfer", ({ payload }) => handler(payload));

export const fetchSystemMetrics''',
    '''export const listenSftpTransfers = (handler: (event: TransferEvent) => void): Promise<UnlistenFn> =>
  listen<TransferEvent>("sftp-transfer", ({ payload }) => handler(payload));

export const listenSftpQueueTasks = (handler: (task: TransferQueueTask) => void): Promise<UnlistenFn> =>
  listen<TransferQueueTask>("sftp-queue-task", ({ payload }) => handler(payload));

export const fetchSystemMetrics''',
    "queue task listener",
)

service_path.write_text(service, encoding="utf-8")
