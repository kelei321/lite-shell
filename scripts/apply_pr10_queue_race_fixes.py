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
    "sync::{Mutex as AsyncMutex, Notify, OnceCell},",
    "sync::{broadcast, Mutex as AsyncMutex, Notify, OnceCell},",
    "broadcast import",
)
queue = replace_once(
    queue,
    "static ID_COUNTER: AtomicU64 = AtomicU64::new(1);",
    "static ID_COUNTER: AtomicU64 = AtomicU64::new(1);\nstatic QUEUE_CLOCK: AtomicU64 = AtomicU64::new(0);",
    "queue clock",
)
queue = replace_once(
    queue,
    '''pub struct TransferQueueSnapshot {
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
}''',
    '''pub struct TransferQueueSnapshot {
    generated_at: u64,
    concurrency: u8,
    tasks: Vec<TransferQueueTask>,
}''',
    "snapshot generation",
)
queue = replace_once(
    queue,
    '''    ensure_initialized(&app, &queue).await?;
    let (concurrency, mut tasks) = {
        let inner = queue.inner.lock().await;
        (inner.concurrency, inner.tasks.clone())
    };''',
    '''    ensure_initialized(&app, &queue).await?;
    let generated_at = unix_now();
    let (concurrency, mut tasks) = {
        let inner = queue.inner.lock().await;
        (inner.concurrency, inner.tasks.clone())
    };''',
    "snapshot timestamp capture",
)
queue = replace_once(
    queue,
    "    Ok(TransferQueueSnapshot { concurrency, tasks })",
    '''    Ok(TransferQueueSnapshot {
        generated_at,
        concurrency,
        tasks,
    })''',
    "snapshot response",
)
queue = replace_once(
    queue,
    '''            let progress_app = app.clone();
            tauri::async_runtime::spawn(async move {
                progress_loop(progress_app).await;
            });
            let dispatcher_app = app.clone();''',
    '''            let progress_receiver = app.state::<SftpTransferManager>().subscribe();
            let progress_app = app.clone();
            tauri::async_runtime::spawn(async move {
                progress_loop(progress_app, progress_receiver).await;
            });
            let dispatcher_app = app.clone();''',
    "subscribe before dispatch",
)
queue = replace_once(
    queue,
    '''async fn progress_loop(app: AppHandle) {
    let mut receiver = app.state::<SftpTransferManager>().subscribe();
    loop {''',
    '''async fn progress_loop(
    app: AppHandle,
    mut receiver: broadcast::Receiver<TransferEvent>,
) {
    loop {''',
    "progress receiver argument",
)
queue = replace_once(
    queue,
    '''fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}''',
    '''fn unix_now() -> u64 {
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
}''',
    "monotonic microsecond clock",
)
queue = replace_once(
    queue,
    '''    #[test]
    fn queue_preserves_fifo_order_and_concurrency_limit() {''',
    '''    #[test]
    fn queue_timestamps_are_strictly_monotonic() {
        let first = unix_now();
        let second = unix_now();
        assert!(second > first);
    }

    #[test]
    fn queue_preserves_fifo_order_and_concurrency_limit() {''',
    "timestamp test",
)
queue_path.write_text(queue, encoding="utf-8")

service_path = Path("src/services/ssh.ts")
service = service_path.read_text(encoding="utf-8")
service = replace_once(
    service,
    '''export type TransferQueueSnapshot = {
  concurrency: number;
  tasks: TransferQueueTask[];
};''',
    '''export type TransferQueueSnapshot = {
  generatedAt: number;
  concurrency: number;
  tasks: TransferQueueTask[];
};''',
    "frontend snapshot timestamp",
)
service_path.write_text(service, encoding="utf-8")

controller_path = Path("src/sftp/transfer-queue.ts")
controller = controller_path.read_text(encoding="utf-8")
controller = replace_once(
    controller,
    '''} from "../services/ssh";
''',
    '''} from "../services/ssh";
import {
  mergeTransferQueueSnapshot,
  shouldApplyTransferTask,
} from "./transfer-queue-state";
''',
    "queue state merge import",
)
controller = replace_once(
    controller,
    '''  function handleTransfer(task: TransferQueueTask) {
    const index = transfers.value.findIndex((item) => item.taskId === task.taskId);
    if (index >= 0) transfers.value[index] = task;
    else transfers.value.push(task);
    settleWaiters(task);
  }''',
    '''  function handleTransfer(task: TransferQueueTask) {
    const index = transfers.value.findIndex((item) => item.taskId === task.taskId);
    const current = index >= 0 ? transfers.value[index] : undefined;
    if (!shouldApplyTransferTask(current, task)) return;
    const next = current?.availableSessionId && !task.availableSessionId
      ? { ...task, availableSessionId: current.availableSessionId }
      : task;
    if (index >= 0) transfers.value[index] = next;
    else transfers.value.push(next);
    settleWaiters(next);
  }''',
    "monotonic event upsert",
)
controller = replace_once(
    controller,
    '''    transferConcurrency.value = snapshot.concurrency;
    transfers.value = snapshot.tasks;
    for (const task of snapshot.tasks) settleWaiters(task);''',
    '''    transferConcurrency.value = snapshot.concurrency;
    transfers.value = mergeTransferQueueSnapshot(transfers.value, snapshot);
    for (const task of transfers.value) settleWaiters(task);''',
    "snapshot merge",
)
controller_path.write_text(controller, encoding="utf-8")

app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    '''  if (item.state === "queued") return item.availableSessionId ? "排队中" : "等待连接";''',
    '''  if (item.state === "queued") {
    const connected = Boolean(item.availableSessionId)
      || sessions.value.some((session) => session.connected && session.id === item.sessionId);
    return connected ? "排队中" : "等待连接";
  }''',
    "queued connection status",
)
app_path.write_text(app, encoding="utf-8")
