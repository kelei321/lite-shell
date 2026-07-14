from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    '''  if (session) {
    session.state = event.kind;
    session.connected = event.kind === "connected" || (session.connected && event.kind === "data");
    if (event.kind === "disconnected" || event.kind === "exit" || event.kind === "error") session.connected = false;
  }
  if (event.kind === "connected" && event.sessionId === activeSessionId.value) void refreshMetrics();
''',
    '''  if (session) {
    session.state = event.kind;
    session.connected = event.kind === "connected" || (session.connected && event.kind === "data");
    if (event.kind === "disconnected" || event.kind === "exit" || event.kind === "error") session.connected = false;
  }
  if (["connected", "disconnected", "exit", "error"].includes(event.kind)) void refreshTransferCheckpoints();
  if (event.kind === "connected" && event.sessionId === activeSessionId.value) void refreshMetrics();
''',
    "refresh checkpoints on session state changes",
)
app_path.write_text(app, encoding="utf-8", newline="\n")

sftp_path = Path("src-tauri/src/sftp.rs")
sftp = sftp_path.read_text(encoding="utf-8")
sftp = replace_once(
    sftp,
    '''fn validate_checkpoint_temporary_path(checkpoint: &TransferCheckpoint) -> Result<(), CommandError> {
    let suffix = format!(".liteshell-{}.part", checkpoint.task_id);
    if !checkpoint.temporary_path.ends_with(&suffix) {
        return Err(CommandError::new(
            "TRANSFER_CHECKPOINT_INVALID",
            "检查点临时路径无效",
        ));
    }
    Ok(())
}
''',
    '''fn validate_checkpoint_temporary_path(checkpoint: &TransferCheckpoint) -> Result<(), CommandError> {
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
''',
    "bind temporary path to target",
)
sftp = replace_once(
    sftp,
    '''        second_data[0] = 2;
        second_data[second_data.len() - 1] = 3;
        let mut second = std::io::Cursor::new(second_data);
''',
    '''        second_data[0] = 2;
        let last_index = second_data.len() - 1;
        second_data[last_index] = 3;
        let mut second = std::io::Cursor::new(second_data);
''',
    "avoid overlapping test borrows",
)
sftp = replace_once(
    sftp,
    '''        assert_eq!(
            validate_checkpoint_temporary_path(&invalid)
                .unwrap_err()
                .code,
            "TRANSFER_CHECKPOINT_INVALID"
        );
    }
''',
    '''        assert_eq!(
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
''',
    "test target-bound temporary path",
)
sftp_path.write_text(sftp, encoding="utf-8", newline="\n")

Path("scripts/apply_pr7_followup.py").unlink()
Path(".github/workflows/apply-pr7-followup.yml").unlink()
