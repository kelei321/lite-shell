from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


sftp_path = Path("src-tauri/src/sftp.rs")
sftp = sftp_path.read_text(encoding="utf-8")

sftp = replace_once(
    sftp,
    "sync::{Mutex as AsyncMutex, Semaphore},",
    "sync::{broadcast, Mutex as AsyncMutex, Semaphore},",
    "tokio broadcast import",
)

sftp = replace_once(
    sftp,
    """pub struct SftpTransferManager {
    cancelled: AsyncMutex<HashSet<String>>,
    active_targets: StdMutex<HashSet<TransferTargetKey>>,
    active_tasks: StdMutex<HashSet<String>>,
    slots: Semaphore,
}""",
    """pub struct SftpTransferManager {
    cancelled: AsyncMutex<HashSet<String>>,
    active_targets: StdMutex<HashSet<TransferTargetKey>>,
    active_tasks: StdMutex<HashSet<String>>,
    slots: Semaphore,
    events: broadcast::Sender<TransferEvent>,
}""",
    "transfer manager event sender",
)

sftp = replace_once(
    sftp,
    "#[derive(Debug, Clone, Copy, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum ConflictStrategy",
    "#[derive(Debug, Clone, Copy, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum ConflictStrategy",
    "conflict strategy serialization",
)

sftp = replace_once(
    sftp,
    """pub struct TransferResult {
    path: String,
    skipped: bool,
    resumed_from: u64,
}""",
    """pub struct TransferResult {
    pub(crate) path: String,
    pub(crate) skipped: bool,
    pub(crate) resumed_from: u64,
}""",
    "transfer result visibility",
)

sftp = replace_once(
    sftp,
    """struct TransferEvent {
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
}""",
    """pub(crate) struct TransferEvent {
    pub(crate) transfer_id: String,
    pub(crate) session_id: String,
    pub(crate) direction: &'static str,
    pub(crate) file_name: String,
    pub(crate) transferred: u64,
    pub(crate) total: u64,
    pub(crate) state: &'static str,
    pub(crate) message: Option<String>,
    pub(crate) speed_bytes_per_second: u64,
    pub(crate) eta_seconds: Option<u64>,
    pub(crate) resumed_from: u64,
}""",
    "transfer event visibility",
)

checkpoint_old = """pub struct TransferCheckpoint {
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
    #[serde(skip_deserializing, skip_serializing_if = \"Option::is_none\")]
    available_session_id: Option<String>,
}"""
checkpoint_new = """pub struct TransferCheckpoint {
    pub(crate) version: u8,
    pub(crate) task_id: String,
    pub(crate) session_id: String,
    pub(crate) server_id: String,
    pub(crate) direction: String,
    pub(crate) source_path: String,
    pub(crate) target_path: String,
    pub(crate) source_size: u64,
    pub(crate) source_modified_at: Option<u64>,
    #[serde(default)]
    pub(crate) source_fingerprint: String,
    pub(crate) temporary_path: String,
    pub(crate) transferred: u64,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
    #[serde(skip_deserializing, skip_serializing_if = \"Option::is_none\")]
    pub(crate) available_session_id: Option<String>,
}"""
sftp = replace_once(sftp, checkpoint_old, checkpoint_new, "checkpoint visibility")

sftp = replace_once(
    sftp,
    """    pub(crate) async fn finish_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.remove(operation_id);
    }
}""",
    """    pub(crate) async fn finish_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.remove(operation_id);
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<TransferEvent> {
        self.events.subscribe()
    }
}""",
    "transfer event subscription",
)

sftp = replace_once(
    sftp,
    """impl Default for SftpTransferManager {
    fn default() -> Self {
        Self {
            cancelled: AsyncMutex::new(HashSet::new()),
            active_targets: StdMutex::new(HashSet::new()),
            active_tasks: StdMutex::new(HashSet::new()),
            slots: Semaphore::new(3),
        }
    }
}""",
    """impl Default for SftpTransferManager {
    fn default() -> Self {
        let (events, _) = broadcast::channel(1_024);
        Self {
            cancelled: AsyncMutex::new(HashSet::new()),
            active_targets: StdMutex::new(HashSet::new()),
            active_tasks: StdMutex::new(HashSet::new()),
            slots: Semaphore::new(5),
            events,
        }
    }
}""",
    "transfer manager default",
)

emit_pattern = re.compile(
    r"fn emit_transfer\(\n(?P<signature>.*?)\n\) \{\n    let _ = app\.emit\(\n        \"sftp-transfer\",\n        TransferEvent \{\n(?P<fields>.*?)\n        \},\n    \);\n\}",
    re.S,
)
match = emit_pattern.search(sftp)
if not match:
    raise RuntimeError("emit transfer function not found")
replacement = (
    "fn emit_transfer(\n"
    + match.group("signature")
    + "\n) {\n    let event = TransferEvent {\n"
    + match.group("fields")
    + "\n    };\n    let _ = app.emit(\"sftp-transfer\", event.clone());\n"
      "    let _ = app.state::<SftpTransferManager>().events.send(event);\n}"
)
sftp = sftp[: match.start()] + replacement + sftp[match.end() :]
sftp_path.write_text(sftp, encoding="utf-8")

queue_path = Path("src-tauri/src/sftp_queue.rs")
queue = queue_path.read_text(encoding="utf-8")
queue = replace_once(
    queue,
    """    #[serde(skip)]
    available_session_id: Option<String>,""",
    """    #[serde(skip_deserializing, skip_serializing_if = \"Option::is_none\")]
    available_session_id: Option<String>,""",
    "available session serialization",
)

queue = replace_once(
    queue,
    """        inner.tasks[index].updated_at = unix_now();
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_retry""",
    """        inner.tasks[index].updated_at = unix_now();
        inner.restored_waiting.insert(task_id.clone());
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_retry""",
    "resume waits for server",
)

queue = replace_once(
    queue,
    """        inner.tasks[index].updated_at = unix_now();
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_cancel""",
    """        inner.tasks[index].updated_at = unix_now();
        inner.restored_waiting.insert(task_id.clone());
        inner.tasks[index].clone()
    };
    persist_current(&app, &queue).await?;
    emit_task(&app, &task);
    queue.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn sftp_queue_cancel""",
    "retry waits for server",
)

missing_session_pattern = re.compile(
    r"        let Some\(session_id\) = session_id else \{\n(?P<body>.*?)\n            continue;\n        \};",
    re.S,
)
match = missing_session_pattern.search(queue)
if not match:
    raise RuntimeError("missing-session dispatch branch not found")
missing_session_replacement = """        let Some(session_id) = session_id else {
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
        };"""
queue = queue[: match.start()] + missing_session_replacement + queue[match.end() :]

queue = replace_once(
    queue,
    """        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        inner.tasks[index].state = QueueTaskState::Completed;""",
    """        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        inner.tasks[index].attempt_id = None;
        inner.tasks[index].state = QueueTaskState::Completed;""",
    "clear successful attempt",
)

queue = replace_once(
    queue,
    """        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        match action {""",
    """        inner.running.remove(task_id);
        inner.actions.remove(task_id);
        inner.tasks[index].attempt_id = None;
        match action {""",
    "clear failed attempt",
)

queue = replace_once(
    queue,
    """            Some(RequestedAction::Pause) => {
                inner.tasks[index].state = QueueTaskState::Paused;
                inner.tasks[index].message = Some("任务已安全暂停".to_owned());
            }
            Some(RequestedAction::CancelKeep) => {
                inner.tasks[index].state = QueueTaskState::Cancelled;
                inner.tasks[index].message = Some("任务已取消，断点已保留".to_owned());
            }""",
    """            Some(RequestedAction::Pause) => {
                inner.tasks[index].state = QueueTaskState::Paused;
                inner.tasks[index].checkpoint_available = true;
                inner.tasks[index].message = Some("任务已安全暂停".to_owned());
            }
            Some(RequestedAction::CancelKeep) => {
                inner.tasks[index].state = QueueTaskState::Cancelled;
                inner.tasks[index].checkpoint_available = true;
                inner.tasks[index].message = Some("任务已取消，断点已保留".to_owned());
            }""",
    "paused checkpoint state",
)

queue_path.write_text(queue, encoding="utf-8")
