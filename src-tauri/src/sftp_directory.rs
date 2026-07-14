use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use russh_sftp::{
    client::{error::Error as SftpClientError, SftpSession},
    protocol::StatusCode,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::fs;

use crate::ssh::{open_sftp, session_server_id, CommandError, SessionManager};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryConflictStrategy {
    Merge,
    Skip,
    Rename,
    Replace,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryPrepareResult {
    path: String,
    skipped: bool,
    existed: bool,
    replacement_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPathInspection {
    kind: &'static str,
}

#[derive(Debug, Clone)]
enum DirectoryReplacement {
    Local {
        target: PathBuf,
        backup: PathBuf,
    },
    Remote {
        server_id: String,
        target: String,
        backup: String,
    },
}

#[derive(Default)]
pub struct DirectoryReplacementManager {
    transactions: Mutex<HashMap<String, DirectoryReplacement>>,
}

impl DirectoryReplacementManager {
    fn ensure_available(&self, replacement_id: &str) -> Result<(), CommandError> {
        validate_replacement_id(replacement_id)?;
        let transactions = self.transactions.lock().map_err(|_| {
            CommandError::new(
                "DIRECTORY_REPLACEMENT_LOCK_FAILED",
                "目录替换状态不可用，请稍后重试",
            )
        })?;
        if transactions.contains_key(replacement_id) {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        Ok(())
    }

    fn register(
        &self,
        replacement_id: &str,
        replacement: DirectoryReplacement,
    ) -> Result<(), CommandError> {
        let mut transactions = self.transactions.lock().map_err(|_| {
            CommandError::new(
                "DIRECTORY_REPLACEMENT_LOCK_FAILED",
                "目录替换状态不可用，请稍后重试",
            )
        })?;
        if transactions
            .insert(replacement_id.to_owned(), replacement)
            .is_some()
        {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        Ok(())
    }

    fn get(&self, replacement_id: &str) -> Result<DirectoryReplacement, CommandError> {
        validate_replacement_id(replacement_id)?;
        self.transactions
            .lock()
            .map_err(|_| {
                CommandError::new(
                    "DIRECTORY_REPLACEMENT_LOCK_FAILED",
                    "目录替换状态不可用，请稍后重试",
                )
            })?
            .get(replacement_id)
            .cloned()
            .ok_or_else(|| {
                CommandError::new(
                    "DIRECTORY_REPLACEMENT_NOT_FOUND",
                    "目录替换任务不存在或已经结束",
                )
            })
    }

    fn remove(&self, replacement_id: &str) {
        if let Ok(mut transactions) = self.transactions.lock() {
            transactions.remove(replacement_id);
        }
    }
}

#[tauri::command]
pub async fn sftp_inspect_local_path(path: String) -> Result<LocalPathInspection, CommandError> {
    if path.trim().is_empty() {
        return Err(CommandError::new("LOCAL_PATH_INVALID", "本地路径不能为空"));
    }
    match fs::symlink_metadata(path.trim()).await {
        Ok(metadata) if is_local_link_or_reparse(&metadata) => {
            Ok(LocalPathInspection { kind: "other" })
        }
        Ok(metadata) if metadata.is_dir() => Ok(LocalPathInspection { kind: "directory" }),
        Ok(metadata) if metadata.is_file() => Ok(LocalPathInspection { kind: "file" }),
        Ok(_) => Ok(LocalPathInspection { kind: "other" }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(LocalPathInspection { kind: "missing" })
        }
        Err(error) => Err(CommandError::new(
            "LOCAL_PATH_INSPECTION_FAILED",
            error.to_string(),
        )),
    }
}

#[tauri::command]
pub async fn sftp_prepare_local_directory(
    replacements: State<'_, DirectoryReplacementManager>,
    path: String,
    conflict_strategy: DirectoryConflictStrategy,
    replacement_id: Option<String>,
) -> Result<DirectoryPrepareResult, CommandError> {
    prepare_local_directory(
        &replacements,
        path.trim(),
        conflict_strategy,
        replacement_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn sftp_prepare_remote_directory(
    manager: State<'_, SessionManager>,
    replacements: State<'_, DirectoryReplacementManager>,
    session_id: String,
    path: String,
    conflict_strategy: DirectoryConflictStrategy,
    replacement_id: Option<String>,
) -> Result<DirectoryPrepareResult, CommandError> {
    validate_remote_directory_path(&path)?;
    let server_id = session_server_id(&manager, &session_id).await?;
    let sftp = open_sftp(&manager, &session_id).await?;
    let result = prepare_remote_directory(
        &sftp,
        &replacements,
        &server_id,
        path.trim(),
        conflict_strategy,
        replacement_id.as_deref(),
    )
    .await;
    sftp.close().await.ok();
    result
}

#[tauri::command]
pub async fn sftp_finish_directory_replacement(
    manager: State<'_, SessionManager>,
    replacements: State<'_, DirectoryReplacementManager>,
    replacement_id: String,
    commit: bool,
    session_id: Option<String>,
) -> Result<(), CommandError> {
    let replacement = replacements.get(&replacement_id)?;
    let result = match replacement {
        DirectoryReplacement::Local { target, backup } => {
            finish_local_replacement(&target, &backup, commit).await
        }
        DirectoryReplacement::Remote {
            server_id,
            target,
            backup,
        } => {
            let session_id = session_id.ok_or_else(|| {
                CommandError::new(
                    "DIRECTORY_REPLACEMENT_SESSION_REQUIRED",
                    "远程目录替换需要重新连接对应服务器",
                )
            })?;
            let current_server_id = session_server_id(&manager, &session_id).await?;
            if current_server_id != server_id {
                return Err(CommandError::new(
                    "DIRECTORY_REPLACEMENT_SERVER_MISMATCH",
                    "当前 SSH 会话与目录替换所属服务器不匹配",
                ));
            }
            let sftp = open_sftp(&manager, &session_id).await?;
            let result = finish_remote_replacement(&sftp, &target, &backup, commit).await;
            sftp.close().await.ok();
            result
        }
    };
    if result.is_ok() {
        replacements.remove(&replacement_id);
    }
    result
}

async fn prepare_local_directory(
    replacements: &DirectoryReplacementManager,
    path: &str,
    strategy: DirectoryConflictStrategy,
    replacement_id: Option<&str>,
) -> Result<DirectoryPrepareResult, CommandError> {
    if path.is_empty() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不能为空",
        ));
    }
    let target = PathBuf::from(path);
    let existing = match fs::symlink_metadata(&target).await {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(CommandError::new(
                "LOCAL_DIRECTORY_READ_FAILED",
                error.to_string(),
            ))
        }
    };
    if let Some(metadata) = &existing {
        if is_local_link_or_reparse(metadata) {
            return Err(CommandError::new(
                "LOCAL_DIRECTORY_LINK_UNSUPPORTED",
                "目标目录是符号链接或 Windows junction，不能执行目录冲突操作",
            ));
        }
        if !metadata.is_dir() {
            return Err(CommandError::new(
                "LOCAL_TARGET_IS_FILE",
                "目标路径已存在同名文件",
            ));
        }
    }
    let existed = existing.is_some();
    if !existed {
        fs::create_dir_all(&target).await.map_err(|error| {
            CommandError::new("LOCAL_DIRECTORY_CREATE_FAILED", error.to_string())
        })?;
        return Ok(DirectoryPrepareResult {
            path: target.to_string_lossy().into_owned(),
            skipped: false,
            existed: false,
            replacement_id: None,
        });
    }

    match strategy {
        DirectoryConflictStrategy::Merge => Ok(DirectoryPrepareResult {
            path: target.to_string_lossy().into_owned(),
            skipped: false,
            existed: true,
            replacement_id: None,
        }),
        DirectoryConflictStrategy::Skip => Ok(DirectoryPrepareResult {
            path: target.to_string_lossy().into_owned(),
            skipped: true,
            existed: true,
            replacement_id: None,
        }),
        DirectoryConflictStrategy::Rename => {
            let renamed = unique_local_directory_path(&target).await?;
            fs::create_dir_all(&renamed).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_CREATE_FAILED", error.to_string())
            })?;
            Ok(DirectoryPrepareResult {
                path: renamed.to_string_lossy().into_owned(),
                skipped: false,
                existed: true,
                replacement_id: None,
            })
        }
        DirectoryConflictStrategy::Replace => {
            let replacement_id = replacement_id.ok_or_else(|| {
                CommandError::new(
                    "INVALID_DIRECTORY_REPLACEMENT",
                    "目录替换标识不能为空",
                )
            })?;
            replacements.ensure_available(replacement_id)?;
            let backup = local_backup_path(&target, replacement_id)?;
            if fs::symlink_metadata(&backup).await.is_ok() {
                return Err(CommandError::new(
                    "LOCAL_DIRECTORY_BACKUP_EXISTS",
                    "目录替换备份路径已经存在，请先处理残留备份",
                ));
            }
            fs::rename(&target, &backup).await.map_err(|_| {
                CommandError::new(
                    "LOCAL_DIRECTORY_REPLACE_UNSUPPORTED",
                    "无法安全重命名原目录，已取消替换且不会删除原目录",
                )
            })?;
            let replacement = DirectoryReplacement::Local {
                target: target.clone(),
                backup: backup.clone(),
            };
            if let Err(error) = replacements.register(replacement_id, replacement) {
                fs::rename(&backup, &target).await.ok();
                return Err(error);
            }
            if let Err(error) = fs::create_dir_all(&target).await {
                replacements.remove(replacement_id);
                let restored = fs::rename(&backup, &target).await;
                return Err(if restored.is_ok() {
                    CommandError::new("LOCAL_DIRECTORY_CREATE_FAILED", error.to_string())
                } else {
                    CommandError::new(
                        "LOCAL_DIRECTORY_RESTORE_FAILED",
                        "创建新目录失败，且原目录自动恢复失败；备份目录已保留",
                    )
                });
            }
            Ok(DirectoryPrepareResult {
                path: target.to_string_lossy().into_owned(),
                skipped: false,
                existed: true,
                replacement_id: Some(replacement_id.to_owned()),
            })
        }
    }
}

async fn prepare_remote_directory(
    sftp: &SftpSession,
    replacements: &DirectoryReplacementManager,
    server_id: &str,
    path: &str,
    strategy: DirectoryConflictStrategy,
    replacement_id: Option<&str>,
) -> Result<DirectoryPrepareResult, CommandError> {
    let existing = remote_metadata(sftp, path).await?;
    if let Some(metadata) = &existing {
        if !metadata.is_dir() {
            return Err(CommandError::new(
                "SFTP_TARGET_IS_FILE",
                "目标路径已存在同名文件",
            ));
        }
    }
    let existed = existing.is_some();
    if !existed {
        sftp.create_dir(path.to_owned())
            .await
            .map_err(sftp_error("SFTP_CREATE_DIRECTORY_FAILED"))?;
        return Ok(DirectoryPrepareResult {
            path: path.to_owned(),
            skipped: false,
            existed: false,
            replacement_id: None,
        });
    }

    match strategy {
        DirectoryConflictStrategy::Merge => Ok(DirectoryPrepareResult {
            path: path.to_owned(),
            skipped: false,
            existed: true,
            replacement_id: None,
        }),
        DirectoryConflictStrategy::Skip => Ok(DirectoryPrepareResult {
            path: path.to_owned(),
            skipped: true,
            existed: true,
            replacement_id: None,
        }),
        DirectoryConflictStrategy::Rename => {
            let renamed = unique_remote_directory_path(sftp, path).await?;
            sftp.create_dir(renamed.clone())
                .await
                .map_err(sftp_error("SFTP_CREATE_DIRECTORY_FAILED"))?;
            Ok(DirectoryPrepareResult {
                path: renamed,
                skipped: false,
                existed: true,
                replacement_id: None,
            })
        }
        DirectoryConflictStrategy::Replace => {
            let replacement_id = replacement_id.ok_or_else(|| {
                CommandError::new(
                    "INVALID_DIRECTORY_REPLACEMENT",
                    "目录替换标识不能为空",
                )
            })?;
            replacements.ensure_available(replacement_id)?;
            let backup = remote_backup_path(path, replacement_id)?;
            if remote_metadata(sftp, &backup).await?.is_some() {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_BACKUP_EXISTS",
                    "远程目录替换备份路径已经存在，请先处理残留备份",
                ));
            }
            sftp.rename(path.to_owned(), backup.clone())
                .await
                .map_err(|_| {
                    CommandError::new(
                        "SFTP_DIRECTORY_REPLACE_UNSUPPORTED",
                        "服务器不支持安全目录重命名，已取消替换且不会递归删除原目录",
                    )
                })?;
            let replacement = DirectoryReplacement::Remote {
                server_id: server_id.to_owned(),
                target: path.to_owned(),
                backup: backup.clone(),
            };
            if let Err(error) = replacements.register(replacement_id, replacement) {
                sftp.rename(backup, path.to_owned()).await.ok();
                return Err(error);
            }
            if let Err(error) = sftp.create_dir(path.to_owned()).await {
                replacements.remove(replacement_id);
                let restored = sftp.rename(backup, path.to_owned()).await;
                return Err(if restored.is_ok() {
                    sftp_error("SFTP_CREATE_DIRECTORY_FAILED")(error)
                } else {
                    CommandError::new(
                        "SFTP_DIRECTORY_RESTORE_FAILED",
                        "创建新目录失败，且原目录自动恢复失败；远程备份目录已保留",
                    )
                });
            }
            Ok(DirectoryPrepareResult {
                path: path.to_owned(),
                skipped: false,
                existed: true,
                replacement_id: Some(replacement_id.to_owned()),
            })
        }
    }
}

async fn finish_local_replacement(
    target: &Path,
    backup: &Path,
    commit: bool,
) -> Result<(), CommandError> {
    if commit {
        fs::remove_dir_all(backup).await.map_err(|error| {
            CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
        })?;
        return Ok(());
    }
    match fs::remove_dir_all(target).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(CommandError::new(
                "LOCAL_DIRECTORY_ROLLBACK_DELETE_FAILED",
                error.to_string(),
            ))
        }
    }
    fs::rename(backup, target).await.map_err(|error| {
        CommandError::new("LOCAL_DIRECTORY_RESTORE_FAILED", error.to_string())
    })
}

async fn finish_remote_replacement(
    sftp: &SftpSession,
    target: &str,
    backup: &str,
    commit: bool,
) -> Result<(), CommandError> {
    if commit {
        return remove_remote_directory_recursive(sftp, backup).await;
    }
    remove_remote_directory_recursive(sftp, target).await?;
    sftp.rename(backup.to_owned(), target.to_owned())
        .await
        .map_err(sftp_error("SFTP_DIRECTORY_RESTORE_FAILED"))
}

async fn remove_remote_directory_recursive(
    sftp: &SftpSession,
    path: &str,
) -> Result<(), CommandError> {
    validate_remote_directory_path(path)?;
    let root = sftp
        .canonicalize(path.to_owned())
        .await
        .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_PATH_FAILED"))?;
    validate_remote_directory_path(&root)?;
    let root_prefix = format!("{}/", root.trim_end_matches('/'));
    let mut stack = vec![(root, false)];
    while let Some((current, visited)) = stack.pop() {
        if visited {
            sftp.remove_dir(current)
                .await
                .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_DELETE_FAILED"))?;
            continue;
        }
        stack.push((current.clone(), true));
        let entries = sftp
            .read_dir(current)
            .await
            .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_LIST_FAILED"))?;
        for entry in entries {
            let name = entry.file_name();
            if matches!(name.as_str(), "." | "..") {
                continue;
            }
            let child = entry.path();
            if !child.starts_with(&root_prefix) {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_REPLACE_UNSAFE_PATH",
                    "服务器返回了替换目录之外的路径，已停止操作",
                ));
            }
            let file_type = entry.file_type();
            if file_type.is_dir() && !file_type.is_symlink() {
                stack.push((child, false));
            } else {
                sftp.remove_file(child)
                    .await
                    .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_DELETE_FAILED"))?;
            }
        }
    }
    Ok(())
}

async fn remote_metadata(
    sftp: &SftpSession,
    path: &str,
) -> Result<Option<std::fs::Metadata>, CommandError> {
    match sftp.metadata(path.to_owned()).await {
        Ok(metadata) => Ok(Some(metadata)),
        Err(SftpClientError::Status(status))
            if status.status_code == StatusCode::NoSuchFile =>
        {
            Ok(None)
        }
        Err(error) => Err(sftp_error("SFTP_DIRECTORY_READ_FAILED")(error)),
    }
}

async fn unique_local_directory_path(path: &Path) -> Result<PathBuf, CommandError> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("directory");
    for index in 1..10_000 {
        let candidate = parent.join(format!("{name} ({index})"));
        match fs::symlink_metadata(&candidate).await {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => {
                return Err(CommandError::new(
                    "LOCAL_DIRECTORY_READ_FAILED",
                    error.to_string(),
                ))
            }
        }
    }
    Err(CommandError::new(
        "LOCAL_DIRECTORY_RENAME_EXHAUSTED",
        "无法生成可用的目录重命名路径",
    ))
}

async fn unique_remote_directory_path(
    sftp: &SftpSession,
    path: &str,
) -> Result<String, CommandError> {
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    for index in 1..10_000 {
        let candidate_name = format!("{name} ({index})");
        let candidate = if parent.is_empty() {
            candidate_name
        } else if parent == "/" {
            format!("/{candidate_name}")
        } else {
            format!("{parent}/{candidate_name}")
        };
        if remote_metadata(sftp, &candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    Err(CommandError::new(
        "SFTP_DIRECTORY_RENAME_EXHAUSTED",
        "无法生成可用的远程目录重命名路径",
    ))
}

fn local_backup_path(target: &Path, replacement_id: &str) -> Result<PathBuf, CommandError> {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别目录名称"))?;
    Ok(target.with_file_name(format!(
        "{name}.liteshell-dir-backup-{replacement_id}"
    )))
}

fn remote_backup_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
    validate_remote_directory_path(path)?;
    Ok(format!(
        "{}.liteshell-dir-backup-{replacement_id}",
        path.trim_end_matches('/')
    ))
}

fn validate_replacement_id(replacement_id: &str) -> Result<(), CommandError> {
    if replacement_id.is_empty()
        || !replacement_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        return Err(CommandError::new(
            "INVALID_DIRECTORY_REPLACEMENT",
            "目录替换标识无效",
        ));
    }
    Ok(())
}

fn validate_remote_directory_path(path: &str) -> Result<(), CommandError> {
    let path = path.trim();
    if path.is_empty() || matches!(path, "/" | "." | "..") || path.contains('\0') {
        return Err(CommandError::new(
            "INVALID_REMOTE_PATH",
            "不能修改空路径、根目录或相对父目录",
        ));
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

fn sftp_error(code: &'static str) -> impl FnOnce(SftpClientError) -> CommandError {
    move |error| {
        let message = match &error {
            SftpClientError::Status(status) => match status.status_code {
                StatusCode::PermissionDenied => "权限不足，请检查当前用户及父目录权限".to_owned(),
                StatusCode::NoSuchFile => "文件或目录不存在，可能已被其他操作删除".to_owned(),
                StatusCode::Failure => {
                    "服务器拒绝了操作，目录可能非空、正在使用或权限不足".to_owned()
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

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "liteshell-directory-strategy-{}-{name}",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn merge_preserves_existing_local_directory_contents() {
        let target = test_path("merge");
        fs::create_dir_all(&target).await.unwrap();
        fs::write(target.join("extra.txt"), b"extra").await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let result = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Merge,
            None,
        )
        .await
        .unwrap();
        assert!(result.existed);
        assert!(!result.skipped);
        assert!(target.join("extra.txt").is_file());
        fs::remove_dir_all(target).await.unwrap();
    }

    #[tokio::test]
    async fn skip_does_not_change_existing_local_directory() {
        let target = test_path("skip");
        fs::create_dir_all(&target).await.unwrap();
        fs::write(target.join("keep.txt"), b"keep").await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let result = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Skip,
            None,
        )
        .await
        .unwrap();
        assert!(result.skipped);
        assert!(target.join("keep.txt").is_file());
        fs::remove_dir_all(target).await.unwrap();
    }

    #[tokio::test]
    async fn rename_uses_stable_directory_suffixes() {
        let target = test_path("rename");
        fs::create_dir_all(&target).await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let result = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Rename,
            None,
        )
        .await
        .unwrap();
        assert!(result.path.ends_with("rename (1)"));
        assert!(Path::new(&result.path).is_dir());
        fs::remove_dir_all(target).await.unwrap();
        fs::remove_dir_all(result.path).await.unwrap();
    }

    #[tokio::test]
    async fn rollback_restores_the_original_local_directory() {
        let target = test_path("rollback");
        fs::create_dir_all(&target).await.unwrap();
        fs::write(target.join("old.txt"), b"old").await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let result = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Replace,
            Some("replace-rollback"),
        )
        .await
        .unwrap();
        fs::write(target.join("new.txt"), b"new").await.unwrap();
        let replacement = replacements.get(result.replacement_id.as_deref().unwrap()).unwrap();
        let DirectoryReplacement::Local { target, backup } = replacement else {
            panic!("expected local replacement");
        };
        finish_local_replacement(&target, &backup, false)
            .await
            .unwrap();
        assert!(target.join("old.txt").is_file());
        assert!(!target.join("new.txt").exists());
        fs::remove_dir_all(target).await.unwrap();
    }

    #[tokio::test]
    async fn commit_keeps_new_local_directory_and_removes_backup() {
        let target = test_path("commit");
        fs::create_dir_all(&target).await.unwrap();
        fs::write(target.join("old.txt"), b"old").await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let result = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Replace,
            Some("replace-commit"),
        )
        .await
        .unwrap();
        fs::write(target.join("new.txt"), b"new").await.unwrap();
        let replacement = replacements.get(result.replacement_id.as_deref().unwrap()).unwrap();
        let DirectoryReplacement::Local { target, backup } = replacement else {
            panic!("expected local replacement");
        };
        finish_local_replacement(&target, &backup, true)
            .await
            .unwrap();
        assert!(target.join("new.txt").is_file());
        assert!(!backup.exists());
        fs::remove_dir_all(target).await.unwrap();
    }
}
