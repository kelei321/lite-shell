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
    '''        task.available_session_id = None;
        if let Some(checkpoint) = checkpoint_by_task.get(task.task_id.as_str()) {''',
    '''        task.available_session_id = None;
        task.checkpoint_available = false;
        if let Some(checkpoint) = checkpoint_by_task.get(task.task_id.as_str()) {''',
    "reset restored checkpoint flag",
)

queue = replace_once(
    queue,
    '''            QueueTaskState::Running | QueueTaskState::Pausing => {
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
            }''',
    '''            QueueTaskState::Running | QueueTaskState::Pausing => {
                task.state = if task.allow_pause && task.checkpoint_available {
                    QueueTaskState::Paused
                } else {
                    QueueTaskState::Failed
                };
                task.message = Some(if task.allow_pause && task.checkpoint_available {
                    "应用重启，任务已从真实检查点安全暂停".to_owned()
                } else if task.allow_pause {
                    "应用重启时未找到可恢复断点，任务已标记失败".to_owned()
                } else {
                    "目录批次因应用重启中断，请重新执行整个目录操作".to_owned()
                });
            }''',
    "restart checkpoint truth",
)

queue = replace_once(
    queue,
    '''        pending.message = Some(if delete_partial {
            "任务已取消，断点已删除".to_owned()
        } else {
            "任务已取消，断点已保留".to_owned()
        });''',
    '''        pending.message = Some(if delete_partial {
            "任务已取消，断点已删除".to_owned()
        } else if pending.checkpoint_available {
            "任务已取消，断点已保留".to_owned()
        } else {
            "任务已取消，尚未产生断点".to_owned()
        });''',
    "immediate cancel message",
)

finish_start = queue.index("async fn finish_worker_error(")
finish_end = queue.index("async fn progress_loop(", finish_start)
finish_replacement = '''async fn finish_worker_error(app: &AppHandle, task_id: &str, error: CommandError) {
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
    let has_checkpoint = checkpoint_exists(app, task_id).await;
    let discard_result = if action == Some(RequestedAction::CancelDelete) {
        Some(discard_checkpoint_if_present(app, &task_snapshot).await)
    } else {
        None
    };
    let error_code = error.code;
    let error_message = error.message;
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
                inner.tasks[index].checkpoint_available = has_checkpoint;
                if has_checkpoint {
                    inner.tasks[index].state = QueueTaskState::Paused;
                    inner.tasks[index].message = Some("任务已从真实检查点安全暂停".to_owned());
                } else {
                    inner.tasks[index].state = QueueTaskState::Failed;
                    inner.tasks[index].message = Some(format!(
                        "无法安全暂停，未生成可恢复断点：{error_message}"
                    ));
                }
            }
            Some(RequestedAction::CancelKeep) => {
                inner.tasks[index].state = QueueTaskState::Cancelled;
                inner.tasks[index].checkpoint_available = has_checkpoint;
                inner.tasks[index].message = Some(if has_checkpoint {
                    "任务已取消，断点已保留".to_owned()
                } else {
                    "任务已取消，尚未产生断点".to_owned()
                });
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
                    inner.tasks[index].checkpoint_available = has_checkpoint;
                    inner.tasks[index].message = Some(format!(
                        "任务已停止，但删除断点失败：{}",
                        discard_error.message
                    ));
                }
                None => {}
            },
            None => {
                inner.tasks[index].state = if error_code == "TRANSFER_CANCELLED" {
                    QueueTaskState::Cancelled
                } else {
                    QueueTaskState::Failed
                };
                inner.tasks[index].checkpoint_available = has_checkpoint;
                inner.tasks[index].message = Some(error_message);
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

'''
queue = queue[:finish_start] + finish_replacement + queue[finish_end:]

discard_start = queue.index("async fn discard_checkpoint_if_present(")
discard_end = queue.index("async fn load_queue_store(", discard_start)
discard_replacement = '''async fn checkpoint_exists(app: &AppHandle, task_id: &str) -> bool {
    let Ok(root) = app.path().app_data_dir() else {
        return false;
    };
    fs::metadata(root.join("transfers").join(format!("{task_id}.json")))
        .await
        .is_ok()
}

async fn discard_checkpoint_if_present(
    app: &AppHandle,
    task: &TransferQueueTask,
) -> Result<(), CommandError> {
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

'''
queue = queue[:discard_start] + discard_replacement + queue[discard_end:]

queue = replace_once(
    queue,
    '''    fn runnable_ids(
        tasks: &[TransferQueueTask],''',
    '''    fn checkpoint(id: &str, server: &str) -> TransferCheckpoint {
        TransferCheckpoint {
            version: 2,
            task_id: id.to_owned(),
            session_id: format!("session-{server}"),
            server_id: server.to_owned(),
            direction: "upload".to_owned(),
            source_path: format!("C:\\{id}.txt"),
            target_path: format!("/tmp/{id}.txt"),
            source_size: 100,
            source_modified_at: Some(1),
            source_fingerprint: "fingerprint".to_owned(),
            temporary_path: format!("/tmp/{id}.txt.liteshell-{id}.part"),
            transferred: 42,
            created_at: 1,
            updated_at: 2,
            available_session_id: None,
        }
    }

    fn runnable_ids(
        tasks: &[TransferQueueTask],''',
    "checkpoint test helper",
)
queue = replace_once(
    queue,
    '''        let mut restored = HashSet::new();
        restore_store(&mut store, &[], &mut restored);
        assert_eq!(store.tasks[0].state, QueueTaskState::Paused);''',
    '''        let mut restored = HashSet::new();
        restore_store(&mut store, &[checkpoint("running", "a")], &mut restored);
        assert_eq!(store.tasks[0].state, QueueTaskState::Paused);''',
    "restart test real checkpoint",
)
queue = replace_once(
    queue,
    '''    #[test]
    fn paused_task_progress_remains_stable() {''',
    '''    #[test]
    fn restart_marks_running_without_checkpoint_failed() {
        let mut store = QueueStore {
            version: QUEUE_VERSION,
            concurrency: 3,
            tasks: vec![task("running", "a", QueueTaskState::Running, 1)],
        };
        let mut restored = HashSet::new();
        restore_store(&mut store, &[], &mut restored);
        assert_eq!(store.tasks[0].state, QueueTaskState::Failed);
        assert!(!store.tasks[0].checkpoint_available);
    }

    #[test]
    fn paused_task_progress_remains_stable() {''',
    "restart no checkpoint test",
)
queue_path.write_text(queue, encoding="utf-8")

lib_path = Path("src-tauri/src/lib.rs")
lib = lib_path.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    '''use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive,
    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_rename, sftp_upload, SftpTransferManager,
};''',
    '''use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive, sftp_list,
    sftp_rename, SftpTransferManager,
};''',
    "remove direct transfer imports",
)
for command in [
    "            sftp_upload,\n",
    "            sftp_download,\n",
    "            sftp_list_transfer_checkpoints,\n",
    "            sftp_delete_transfer_checkpoint,\n",
    "            sftp_discard_transfer_checkpoint,\n",
]:
    lib = replace_once(lib, command, "", f"remove handler {command.strip()}")
lib_path.write_text(lib, encoding="utf-8")

service_path = Path("src/services/ssh.ts")
service = service_path.read_text(encoding="utf-8")
legacy_types_start = service.index("export type TransferCheckpoint = {")
legacy_types_end = service.index("export type ConflictStrategy =", legacy_types_start)
service = service[:legacy_types_start] + service[legacy_types_end:]
transfer_result_start = service.index("export type TransferResult = {")
transfer_result_end = service.index("export type DiskMetrics =", transfer_result_start)
service = service[:transfer_result_start] + service[transfer_result_end:]
legacy_commands_start = service.index("export const uploadSftpFile =")
legacy_commands_end = service.index("export const listSftpTransferQueue =", legacy_commands_start)
service = service[:legacy_commands_start] + service[legacy_commands_end:]
legacy_listener = '''export const listenSftpTransfers = (handler: (event: TransferEvent) => void): Promise<UnlistenFn> =>
  listen<TransferEvent>("sftp-transfer", ({ payload }) => handler(payload));

'''
service = replace_once(service, legacy_listener, "", "remove raw transfer listener")
service_path.write_text(service, encoding="utf-8")
