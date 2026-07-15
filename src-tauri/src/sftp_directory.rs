use std::{
    collections::{HashMap, HashSet},
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
    size: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePathInspection {
    kind: &'static str,
}

#[derive(Debug, Clone)]
enum DirectoryReplacement {
    Local {
        target: PathBuf,
        staging: PathBuf,
        backup: PathBuf,
    },
    Remote {
        server_id: String,
        target: String,
        staging: String,
        backup: String,
    },
}

fn replacements_share_target(left: &DirectoryReplacement, right: &DirectoryReplacement) -> bool {
    match (left, right) {
        (
            DirectoryReplacement::Local { target: left, .. },
            DirectoryReplacement::Local { target: right, .. },
        ) => normalize_local_replacement_target(left) == normalize_local_replacement_target(right),
        (
            DirectoryReplacement::Remote {
                server_id: left_server,
                target: left_target,
                ..
            },
            DirectoryReplacement::Remote {
                server_id: right_server,
                target: right_target,
                ..
            },
        ) => {
            left_server == right_server
                && left_target.trim_end_matches('/') == right_target.trim_end_matches('/')
        }
        _ => false,
    }
}

fn normalize_local_replacement_target(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
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
        if transactions.contains_key(replacement_id) {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        if transactions
            .values()
            .any(|existing| replacements_share_target(existing, &replacement))
        {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_TARGET_BUSY",
                "该目标目录已有替换任务正在运行",
            ));
        }
        transactions.insert(replacement_id.to_owned(), replacement);
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
        Ok(metadata) if is_local_link_or_reparse(&metadata) => Ok(LocalPathInspection {
            kind: "other",
            size: None,
        }),
        Ok(metadata) if metadata.is_dir() => Ok(LocalPathInspection {
            kind: "directory",
            size: None,
        }),
        Ok(metadata) if metadata.is_file() => Ok(LocalPathInspection {
            kind: "file",
            size: Some(metadata.len()),
        }),
        Ok(_) => Ok(LocalPathInspection {
            kind: "other",
            size: None,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LocalPathInspection {
            kind: "missing",
            size: None,
        }),
        Err(error) => Err(CommandError::new(
            "LOCAL_PATH_INSPECTION_FAILED",
            error.to_string(),
        )),
    }
}

#[tauri::command]
pub async fn sftp_inspect_remote_path(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
) -> Result<RemotePathInspection, CommandError> {
    validate_remote_directory_path(&path)?;
    let sftp = open_sftp(&manager, &session_id).await?;
    let result = remote_path_kind(&sftp, path.trim())
        .await
        .map(|kind| RemotePathInspection { kind });
    sftp.close().await.ok();
    result
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
        DirectoryReplacement::Local {
            target,
            staging,
            backup,
        } => finish_local_replacement(&target, &staging, &backup, commit).await,
        DirectoryReplacement::Remote {
            server_id,
            target,
            staging,
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
            let result = finish_remote_replacement(&sftp, &target, &staging, &backup, commit).await;
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
                CommandError::new("INVALID_DIRECTORY_REPLACEMENT", "目录替换标识不能为空")
            })?;
            replacements.ensure_available(replacement_id)?;
            let staging = local_staging_path(&target, replacement_id)?;
            let backup = local_backup_path(&target, replacement_id)?;
            if local_path_exists(&staging).await? || local_path_exists(&backup).await? {
                return Err(CommandError::new(
                    "LOCAL_DIRECTORY_REPLACEMENT_PATH_EXISTS",
                    "目录替换临时路径或备份路径已经存在，请先处理残留目录",
                ));
            }
            fs::create_dir_all(&staging).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_STAGING_CREATE_FAILED", error.to_string())
            })?;
            let replacement = DirectoryReplacement::Local {
                target: target.clone(),
                staging: staging.clone(),
                backup,
            };
            if let Err(error) = replacements.register(replacement_id, replacement) {
                fs::remove_dir_all(&staging).await.ok();
                return Err(error);
            }
            Ok(DirectoryPrepareResult {
                path: staging.to_string_lossy().into_owned(),
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
    let existing_kind = remote_path_kind(sftp, path).await?;
    if !matches!(existing_kind, "missing" | "directory") {
        return Err(CommandError::new(
            "SFTP_TARGET_IS_NOT_DIRECTORY",
            "目标路径已存在同名文件、符号链接或不支持的条目",
        ));
    }
    let existed = existing_kind == "directory";
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
                CommandError::new("INVALID_DIRECTORY_REPLACEMENT", "目录替换标识不能为空")
            })?;
            replacements.ensure_available(replacement_id)?;
            let staging = remote_staging_path(path, replacement_id)?;
            let backup = remote_backup_path(path, replacement_id)?;
            if remote_path_kind(sftp, &staging).await? != "missing"
                || remote_path_kind(sftp, &backup).await? != "missing"
            {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_REPLACEMENT_PATH_EXISTS",
                    "远程目录替换临时路径或备份路径已经存在，请先处理残留目录",
                ));
            }
            sftp.create_dir(staging.clone())
                .await
                .map_err(sftp_error("SFTP_DIRECTORY_STAGING_CREATE_FAILED"))?;
            let replacement = DirectoryReplacement::Remote {
                server_id: server_id.to_owned(),
                target: path.to_owned(),
                staging: staging.clone(),
                backup,
            };
            if let Err(error) = replacements.register(replacement_id, replacement) {
                sftp.remove_dir(staging).await.ok();
                return Err(error);
            }
            Ok(DirectoryPrepareResult {
                path: staging,
                skipped: false,
                existed: true,
                replacement_id: Some(replacement_id.to_owned()),
            })
        }
    }
}

async fn finish_local_replacement(
    target: &Path,
    staging: &Path,
    backup: &Path,
    commit: bool,
) -> Result<(), CommandError> {
    let target_exists = local_path_exists(target).await?;
    let staging_exists = local_path_exists(staging).await?;
    let backup_exists = local_path_exists(backup).await?;

    if !commit {
        if staging_exists {
            fs::remove_dir_all(staging).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_STAGING_DELETE_FAILED", error.to_string())
            })?;
        }
        if backup_exists {
            if target_exists {
                fs::remove_dir_all(target).await.map_err(|error| {
                    CommandError::new("LOCAL_DIRECTORY_ROLLBACK_DELETE_FAILED", error.to_string())
                })?;
            }
            fs::rename(backup, target).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_RESTORE_FAILED", error.to_string())
            })?;
        }
        return Ok(());
    }

    if target_exists && staging_exists && !backup_exists {
        fs::rename(target, backup).await.map_err(|_| {
            CommandError::new(
                "LOCAL_DIRECTORY_REPLACE_UNSUPPORTED",
                "无法安全重命名原目录，原目录保持不变",
            )
        })?;
    }
    let target_exists = local_path_exists(target).await?;
    let staging_exists = local_path_exists(staging).await?;
    let backup_exists = local_path_exists(backup).await?;
    if !target_exists && staging_exists && backup_exists {
        if let Err(error) = fs::rename(staging, target).await {
            let restored = fs::rename(backup, target).await;
            return Err(if restored.is_ok() {
                CommandError::new("LOCAL_DIRECTORY_STAGING_COMMIT_FAILED", error.to_string())
            } else {
                CommandError::new(
                    "LOCAL_DIRECTORY_RESTORE_FAILED",
                    "提交新目录失败，且原目录自动恢复失败；备份目录已保留",
                )
            });
        }
    }
    if local_path_exists(target).await?
        && !local_path_exists(staging).await?
        && local_path_exists(backup).await?
    {
        remove_local_directory_recursive_safe(backup)
            .await
            .map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
            })?;
        return Ok(());
    }
    Err(CommandError::new(
        "LOCAL_DIRECTORY_REPLACEMENT_INVALID_STATE",
        "目录替换状态不完整，已停止提交",
    ))
}

async fn finish_remote_replacement(
    sftp: &SftpSession,
    target: &str,
    staging: &str,
    backup: &str,
    commit: bool,
) -> Result<(), CommandError> {
    let target_kind = remote_path_kind(sftp, target).await?;
    let staging_kind = remote_path_kind(sftp, staging).await?;
    let backup_kind = remote_path_kind(sftp, backup).await?;

    if !commit {
        if staging_kind == "directory" {
            remove_remote_directory_recursive(sftp, staging).await?;
        } else if staging_kind != "missing" {
            return Err(CommandError::new(
                "SFTP_DIRECTORY_STAGING_INVALID",
                "远程替换临时路径不是目录，已停止清理",
            ));
        }
        if backup_kind == "directory" {
            if target_kind == "directory" {
                remove_remote_directory_recursive(sftp, target).await?;
            } else if target_kind != "missing" {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_REPLACEMENT_INVALID_STATE",
                    "远程目标路径类型异常，无法恢复原目录",
                ));
            }
            sftp.rename(backup.to_owned(), target.to_owned())
                .await
                .map_err(sftp_error("SFTP_DIRECTORY_RESTORE_FAILED"))?;
        }
        return Ok(());
    }

    if target_kind == "directory" && staging_kind == "directory" && backup_kind == "missing" {
        sftp.rename(target.to_owned(), backup.to_owned())
            .await
            .map_err(|_| {
                CommandError::new(
                    "SFTP_DIRECTORY_REPLACE_UNSUPPORTED",
                    "服务器不支持安全目录重命名，原目录保持不变",
                )
            })?;
    }
    let target_kind = remote_path_kind(sftp, target).await?;
    let staging_kind = remote_path_kind(sftp, staging).await?;
    let backup_kind = remote_path_kind(sftp, backup).await?;
    if target_kind == "missing" && staging_kind == "directory" && backup_kind == "directory" {
        if let Err(error) = sftp.rename(staging.to_owned(), target.to_owned()).await {
            let restored = sftp.rename(backup.to_owned(), target.to_owned()).await;
            return Err(if restored.is_ok() {
                sftp_error("SFTP_DIRECTORY_STAGING_COMMIT_FAILED")(error)
            } else {
                CommandError::new(
                    "SFTP_DIRECTORY_RESTORE_FAILED",
                    "提交新目录失败，且原目录自动恢复失败；远程备份目录已保留",
                )
            });
        }
    }
    if remote_path_kind(sftp, target).await? == "directory"
        && remote_path_kind(sftp, staging).await? == "missing"
        && remote_path_kind(sftp, backup).await? == "directory"
    {
        remove_remote_directory_recursive(sftp, backup).await?;
        return Ok(());
    }
    Err(CommandError::new(
        "SFTP_DIRECTORY_REPLACEMENT_INVALID_STATE",
        "远程目录替换状态不完整，已停止提交",
    ))
}

async fn remove_local_directory_recursive_safe(path: &Path) -> Result<(), std::io::Error> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if is_local_link_or_reparse(&metadata) {
        return if metadata.is_dir() {
            fs::remove_dir(path).await
        } else {
            fs::remove_file(path).await
        };
    }
    if !metadata.is_dir() {
        return fs::remove_file(path).await;
    }

    let mut stack = vec![(path.to_path_buf(), false, 0_usize)];
    let mut entry_count = 0_usize;
    while let Some((current, post_order, depth)) = stack.pop() {
        if depth > 64 {
            return Err(std::io::Error::other(
                "local directory cleanup exceeded maximum depth 64",
            ));
        }
        if post_order {
            fs::remove_dir(&current).await?;
            continue;
        }
        stack.push((current.clone(), true, depth));
        let mut entries = fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            entry_count = entry_count.saturating_add(1);
            if entry_count > 100_000 {
                return Err(std::io::Error::other(
                    "local directory cleanup exceeded maximum entries 100000",
                ));
            }
            let child = entry.path();
            let metadata = fs::symlink_metadata(&child).await?;
            if is_local_link_or_reparse(&metadata) {
                if metadata.is_dir() {
                    fs::remove_dir(&child).await?;
                } else {
                    fs::remove_file(&child).await?;
                }
            } else if metadata.is_dir() {
                stack.push((child, false, depth + 1));
            } else {
                fs::remove_file(child).await?;
            }
        }
    }
    Ok(())
}

async fn remove_remote_directory_recursive(
    sftp: &SftpSession,
    path: &str,
) -> Result<(), CommandError> {
    validate_remote_directory_path(path)?;
    let kind = remote_path_kind(sftp, path).await?;
    if kind == "missing" {
        return Ok(());
    }
    if kind != "directory" {
        return Err(CommandError::new(
            "SFTP_DIRECTORY_REPLACE_UNSAFE_PATH",
            "目录替换清理目标不是普通目录",
        ));
    }
    let root = sftp
        .canonicalize(path.to_owned())
        .await
        .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_PATH_FAILED"))?;
    validate_remote_directory_path(&root)?;
    let root_prefix = format!("{}/", root.trim_end_matches('/'));
    let mut visited_paths = HashSet::from([root.clone()]);
    let mut stack = vec![(root, false, 0_usize)];
    let mut entry_count = 0_usize;
    while let Some((current, post_order, depth)) = stack.pop() {
        if depth > 64 {
            return Err(CommandError::new(
                "SFTP_DIRECTORY_REPLACE_DEPTH_LIMIT",
                "目录替换清理超过最大安全深度 64",
            ));
        }
        if post_order {
            sftp.remove_dir(current)
                .await
                .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_DELETE_FAILED"))?;
            continue;
        }
        stack.push((current.clone(), true, depth));
        let entries = sftp
            .read_dir(current.clone())
            .await
            .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_LIST_FAILED"))?;
        for entry in entries {
            entry_count = entry_count.saturating_add(1);
            if entry_count > 100_000 {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_REPLACE_ENTRY_LIMIT",
                    "目录替换清理超过最大安全条目数 100000",
                ));
            }
            let name = entry.file_name();
            if !is_safe_remote_entry_name(&name) {
                return Err(CommandError::new(
                    "SFTP_DIRECTORY_REPLACE_UNSAFE_PATH",
                    "服务器返回了异常目录项，已停止清理",
                ));
            }
            let child = join_remote_child(&current, &name);
            let file_type = entry.file_type();
            if file_type.is_dir() && !file_type.is_symlink() {
                let canonical = sftp
                    .canonicalize(child)
                    .await
                    .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_PATH_FAILED"))?;
                if canonical != root_prefix.trim_end_matches('/')
                    && !canonical.starts_with(&root_prefix)
                {
                    return Err(CommandError::new(
                        "SFTP_DIRECTORY_REPLACE_UNSAFE_PATH",
                        "服务器返回了替换目录之外的路径，已停止清理",
                    ));
                }
                if !visited_paths.insert(canonical.clone()) {
                    return Err(CommandError::new(
                        "SFTP_DIRECTORY_REPLACE_CYCLE",
                        "目录替换清理发现重复目录或循环",
                    ));
                }
                stack.push((canonical, false, depth + 1));
            } else {
                sftp.remove_file(child)
                    .await
                    .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_DELETE_FAILED"))?;
            }
        }
    }
    Ok(())
}

async fn remote_path_kind(sftp: &SftpSession, path: &str) -> Result<&'static str, CommandError> {
    let trimmed = path.trim_end_matches('/');
    let (parent, name) = match trimmed.rsplit_once('/') {
        Some(("", name)) => ("/", name),
        Some((parent, name)) => (parent, name),
        None => (".", trimmed),
    };
    if !is_safe_remote_entry_name(name) {
        return Err(CommandError::new("INVALID_REMOTE_PATH", "远程路径名称无效"));
    }
    let canonical_parent = sftp
        .canonicalize(parent)
        .await
        .map_err(sftp_error("SFTP_DIRECTORY_PARENT_FAILED"))?;
    let entries = sftp
        .read_dir(canonical_parent)
        .await
        .map_err(sftp_error("SFTP_DIRECTORY_READ_FAILED"))?;
    for entry in entries {
        if entry.file_name() != name {
            continue;
        }
        let file_type = entry.file_type();
        return Ok(if file_type.is_symlink() {
            "symlink"
        } else if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else {
            "other"
        });
    }
    Ok("missing")
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
        if remote_path_kind(sftp, &candidate).await? == "missing" {
            return Ok(candidate);
        }
    }
    Err(CommandError::new(
        "SFTP_DIRECTORY_RENAME_EXHAUSTED",
        "无法生成可用的远程目录重命名路径",
    ))
}

async fn local_path_exists(path: &Path) -> Result<bool, CommandError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if is_local_link_or_reparse(&metadata) => Err(CommandError::new(
            "LOCAL_DIRECTORY_LINK_UNSUPPORTED",
            "目录替换路径不能是符号链接或 Windows junction",
        )),
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(CommandError::new(
            "LOCAL_DIRECTORY_REPLACEMENT_INVALID_STATE",
            "目录替换路径不是普通目录",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CommandError::new(
            "LOCAL_DIRECTORY_READ_FAILED",
            error.to_string(),
        )),
    }
}

fn local_staging_path(target: &Path, replacement_id: &str) -> Result<PathBuf, CommandError> {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别目录名称"))?;
    Ok(target.with_file_name(format!("{name}.liteshell-dir-staging-{replacement_id}")))
}

fn local_backup_path(target: &Path, replacement_id: &str) -> Result<PathBuf, CommandError> {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别目录名称"))?;
    Ok(target.with_file_name(format!("{name}.liteshell-dir-backup-{replacement_id}")))
}

fn remote_staging_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
    validate_remote_directory_path(path)?;
    Ok(format!(
        "{}.liteshell-dir-staging-{replacement_id}",
        path.trim_end_matches('/')
    ))
}

fn remote_backup_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
    validate_remote_directory_path(path)?;
    Ok(format!(
        "{}.liteshell-dir-backup-{replacement_id}",
        path.trim_end_matches('/')
    ))
}

fn is_safe_remote_entry_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

fn join_remote_child(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    }
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
    if path.is_empty()
        || matches!(path, "/" | "." | "..")
        || path.contains('\0')
        || path.contains('\\')
        || path.split('/').any(|component| component == "..")
    {
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

    #[cfg(unix)]
    #[tokio::test]
    async fn safe_local_cleanup_does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let root = test_path("safe-cleanup");
        let external = test_path("safe-cleanup-external");
        fs::create_dir_all(&root).await.unwrap();
        fs::create_dir_all(&external).await.unwrap();
        fs::write(external.join("keep.txt"), b"keep").await.unwrap();
        symlink(&external, root.join("external-link")).unwrap();
        fs::write(root.join("local.txt"), b"local").await.unwrap();

        remove_local_directory_recursive_safe(&root).await.unwrap();

        assert!(!root.exists());
        assert_eq!(fs::read(external.join("keep.txt")).await.unwrap(), b"keep");
        fs::remove_dir_all(external).await.unwrap();
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
    async fn rollback_discards_staging_and_keeps_the_original_local_directory() {
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
        let staging = PathBuf::from(&result.path);
        fs::write(staging.join("new.txt"), b"new").await.unwrap();
        assert!(target.join("old.txt").is_file());
        let replacement = replacements
            .get(result.replacement_id.as_deref().unwrap())
            .unwrap();
        let DirectoryReplacement::Local {
            target,
            staging,
            backup,
        } = replacement
        else {
            panic!("expected local replacement");
        };
        finish_local_replacement(&target, &staging, &backup, false)
            .await
            .unwrap();
        assert!(target.join("old.txt").is_file());
        assert!(!staging.exists());
        fs::remove_dir_all(target).await.unwrap();
    }

    #[tokio::test]
    async fn commit_swaps_staging_and_removes_the_local_backup() {
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
        let staging_path = PathBuf::from(&result.path);
        fs::write(staging_path.join("new.txt"), b"new")
            .await
            .unwrap();
        let replacement = replacements
            .get(result.replacement_id.as_deref().unwrap())
            .unwrap();
        let DirectoryReplacement::Local {
            target,
            staging,
            backup,
        } = replacement
        else {
            panic!("expected local replacement");
        };
        finish_local_replacement(&target, &staging, &backup, true)
            .await
            .unwrap();
        assert!(target.join("new.txt").is_file());
        assert!(!target.join("old.txt").exists());
        assert!(!staging.exists());
        assert!(!backup.exists());
        fs::remove_dir_all(target).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_two_replacements_for_the_same_local_target() {
        let target = test_path("busy");
        fs::create_dir_all(&target).await.unwrap();
        let replacements = DirectoryReplacementManager::default();
        let first = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Replace,
            Some("replace-busy-one"),
        )
        .await
        .unwrap();
        let second = prepare_local_directory(
            &replacements,
            &target.to_string_lossy(),
            DirectoryConflictStrategy::Replace,
            Some("replace-busy-two"),
        )
        .await
        .unwrap_err();
        assert_eq!(second.code, "DIRECTORY_REPLACEMENT_TARGET_BUSY");
        let replacement = replacements
            .get(first.replacement_id.as_deref().unwrap())
            .unwrap();
        let DirectoryReplacement::Local {
            target,
            staging,
            backup,
        } = replacement
        else {
            panic!("expected local replacement");
        };
        finish_local_replacement(&target, &staging, &backup, false)
            .await
            .unwrap();
        fs::remove_dir_all(target).await.unwrap();
    }
}
