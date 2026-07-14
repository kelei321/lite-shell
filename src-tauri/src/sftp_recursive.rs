use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use russh_sftp::client::SftpSession;
use serde::Serialize;
use tauri::State;
use tokio::fs;

use crate::{
    sftp::SftpTransferManager,
    ssh::{open_sftp, CommandError, SessionManager},
};

const MAX_RECURSIVE_DEPTH: usize = 64;
const MAX_RECURSIVE_FILES: u64 = 100_000;
const MAX_RECURSIVE_DIRECTORIES: u64 = 100_000;
const MAX_RECURSIVE_TOTAL_SIZE: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_SCAN_WARNINGS: usize = 50;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecursiveScanSummary {
    file_count: u64,
    directory_count: u64,
    total_size: u64,
    skipped_links: u64,
    skipped_unsupported: u64,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDirectoryManifest {
    root_name: String,
    directories: Vec<String>,
    files: Vec<LocalManifestFile>,
    #[serde(flatten)]
    summary: RecursiveScanSummary,
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
pub struct RemoteDirectoryManifest {
    root_path: String,
    directories: Vec<String>,
    files: Vec<RemoteManifestFile>,
    #[serde(flatten)]
    summary: RecursiveScanSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteManifestFile {
    remote_path: String,
    relative_path: String,
    size: u64,
}

#[tauri::command]
pub async fn sftp_local_directory_manifest(
    transfers: State<'_, SftpTransferManager>,
    path: String,
    scan_id: String,
) -> Result<LocalDirectoryManifest, CommandError> {
    validate_scan_id(&scan_id)?;
    let result = build_local_directory_manifest(Path::new(path.trim()), &transfers, &scan_id).await;
    transfers.finish_operation(&scan_id).await;
    result
}

#[tauri::command]
pub async fn sftp_remote_directory_manifest(
    manager: State<'_, SessionManager>,
    transfers: State<'_, SftpTransferManager>,
    session_id: String,
    path: String,
    scan_id: String,
) -> Result<RemoteDirectoryManifest, CommandError> {
    validate_scan_id(&scan_id)?;
    let result = async {
        let sftp = open_sftp(&manager, &session_id).await?;
        let result =
            build_remote_directory_manifest(&sftp, path.trim(), &transfers, &scan_id).await;
        sftp.close().await.ok();
        result
    }
    .await;
    transfers.finish_operation(&scan_id).await;
    result
}

async fn build_local_directory_manifest(
    root: &Path,
    transfers: &SftpTransferManager,
    scan_id: &str,
) -> Result<LocalDirectoryManifest, CommandError> {
    if root.as_os_str().is_empty() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不能为空",
        ));
    }
    let root_metadata = fs::symlink_metadata(root)
        .await
        .map_err(|error| CommandError::new("LOCAL_DIRECTORY_INVALID", error.to_string()))?;
    if is_local_link_or_reparse(&root_metadata) {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_LINK_UNSUPPORTED",
            "递归上传不支持符号链接或 Windows junction 根目录",
        ));
    }
    if !root_metadata.is_dir() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不存在",
        ));
    }

    let canonical_root = fs::canonicalize(root)
        .await
        .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))?;
    let root_name = canonical_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别本地目录名称"))?
        .to_owned();
    let normalized_root = normalize_local_boundary(&canonical_root);
    let mut pending = VecDeque::from([(canonical_root.clone(), 0_usize)]);
    let mut visited = HashSet::from([normalized_root.clone()]);
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut summary = RecursiveScanSummary::default();

    while let Some((directory, depth)) = pending.pop_front() {
        ensure_scan_active(transfers, scan_id).await?;
        let mut entries = fs::read_dir(&directory)
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?
        {
            ensure_scan_active(transfers, scan_id).await?;
            let entry_path = entry.path();
            let relative = local_relative_path(&canonical_root, &entry_path)?;
            let metadata = fs::symlink_metadata(&entry_path)
                .await
                .map_err(|error| CommandError::new("LOCAL_ENTRY_READ_FAILED", error.to_string()))?;

            if is_local_link_or_reparse(&metadata) {
                summary.skipped_links += 1;
                push_warning(&mut summary, format!("已跳过链接或 junction：{relative}"));
                continue;
            }

            if metadata.is_dir() {
                let next_depth = depth + 1;
                enforce_depth(next_depth)?;
                let canonical = fs::canonicalize(&entry_path)
                    .await
                    .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))?;
                let normalized = normalize_local_boundary(&canonical);
                if !local_path_within_root(&normalized_root, &normalized) {
                    return Err(CommandError::new(
                        "LOCAL_RECURSIVE_PATH_ESCAPE",
                        "本地目录扫描发现根目录之外的路径",
                    ));
                }
                if !visited.insert(normalized) {
                    summary.skipped_unsupported += 1;
                    push_warning(&mut summary, format!("已跳过重复目录：{relative}"));
                    continue;
                }
                summary.directory_count += 1;
                enforce_directory_count(summary.directory_count)?;
                directories.push(relative);
                pending.push_back((canonical, next_depth));
            } else if metadata.is_file() {
                let canonical = fs::canonicalize(&entry_path)
                    .await
                    .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))?;
                let normalized = normalize_local_boundary(&canonical);
                if !local_path_within_root(&normalized_root, &normalized) {
                    return Err(CommandError::new(
                        "LOCAL_RECURSIVE_PATH_ESCAPE",
                        "本地文件扫描发现根目录之外的路径",
                    ));
                }
                add_file_to_summary(&mut summary, metadata.len())?;
                files.push(LocalManifestFile {
                    absolute_path: canonical.to_string_lossy().into_owned(),
                    relative_path: relative,
                    size: metadata.len(),
                });
            } else {
                summary.skipped_unsupported += 1;
                push_warning(&mut summary, format!("已跳过不支持的本地条目：{relative}"));
            }
        }
    }

    directories.sort();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(LocalDirectoryManifest {
        root_name,
        directories,
        files,
        summary,
    })
}

async fn build_remote_directory_manifest(
    sftp: &SftpSession,
    requested_root: &str,
    transfers: &SftpTransferManager,
    scan_id: &str,
) -> Result<RemoteDirectoryManifest, CommandError> {
    let canonical_root = sftp
        .canonicalize(if requested_root.is_empty() {
            "."
        } else {
            requested_root
        })
        .await
        .map_err(sftp_error("SFTP_RECURSIVE_PATH_FAILED"))?;
    let normalized_root = normalize_remote_path(&canonical_root);
    let mut pending = VecDeque::from([(canonical_root.clone(), String::new(), 0_usize)]);
    let mut visited = HashSet::from([normalized_root.clone()]);
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut summary = RecursiveScanSummary::default();

    while let Some((directory, relative_directory, depth)) = pending.pop_front() {
        ensure_scan_active(transfers, scan_id).await?;
        let entries = sftp
            .read_dir(directory.clone())
            .await
            .map_err(sftp_error("SFTP_RECURSIVE_LIST_FAILED"))?;
        for entry in entries {
            ensure_scan_active(transfers, scan_id).await?;
            let name = entry.file_name();
            if !is_safe_remote_entry_name(&name) {
                summary.skipped_unsupported += 1;
                push_warning(&mut summary, "已跳过服务器返回的异常文件名".to_owned());
                continue;
            }
            let relative = if relative_directory.is_empty() {
                name.clone()
            } else {
                format!("{relative_directory}/{name}")
            };
            let file_type = entry.file_type();
            if file_type.is_symlink() {
                summary.skipped_links += 1;
                push_warning(&mut summary, format!("已跳过远程符号链接：{relative}"));
                continue;
            }

            let candidate = join_remote_child(&directory, &name);
            if file_type.is_dir() {
                let next_depth = depth + 1;
                enforce_depth(next_depth)?;
                let canonical = sftp
                    .canonicalize(candidate)
                    .await
                    .map_err(sftp_error("SFTP_RECURSIVE_PATH_FAILED"))?;
                let normalized = normalize_remote_path(&canonical);
                if !remote_path_within_root(&normalized_root, &normalized) {
                    return Err(CommandError::new(
                        "SFTP_RECURSIVE_PATH_ESCAPE",
                        "服务器返回了所选目录之外的子目录",
                    ));
                }
                if !visited.insert(normalized) {
                    summary.skipped_unsupported += 1;
                    push_warning(&mut summary, format!("已跳过重复远程目录：{relative}"));
                    continue;
                }
                summary.directory_count += 1;
                enforce_directory_count(summary.directory_count)?;
                directories.push(relative.clone());
                pending.push_back((canonical, relative, next_depth));
            } else if file_type.is_file() {
                let canonical = sftp
                    .canonicalize(candidate)
                    .await
                    .map_err(sftp_error("SFTP_RECURSIVE_PATH_FAILED"))?;
                let normalized = normalize_remote_path(&canonical);
                if !remote_path_within_root(&normalized_root, &normalized) {
                    return Err(CommandError::new(
                        "SFTP_RECURSIVE_PATH_ESCAPE",
                        "服务器返回了所选目录之外的文件",
                    ));
                }
                let size = entry.metadata().len();
                add_file_to_summary(&mut summary, size)?;
                files.push(RemoteManifestFile {
                    remote_path: canonical,
                    relative_path: relative,
                    size,
                });
            } else {
                summary.skipped_unsupported += 1;
                push_warning(&mut summary, format!("已跳过不支持的远程条目：{relative}"));
            }
        }
    }

    directories.sort();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(RemoteDirectoryManifest {
        root_path: canonical_root,
        directories,
        files,
        summary,
    })
}

async fn ensure_scan_active(
    transfers: &SftpTransferManager,
    scan_id: &str,
) -> Result<(), CommandError> {
    if transfers.operation_cancelled(scan_id).await {
        Err(CommandError::new(
            "RECURSIVE_SCAN_CANCELLED",
            "目录扫描已取消",
        ))
    } else {
        Ok(())
    }
}

fn validate_scan_id(scan_id: &str) -> Result<(), CommandError> {
    if scan_id.is_empty()
        || !scan_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        return Err(CommandError::new(
            "INVALID_RECURSIVE_SCAN",
            "目录扫描标识无效",
        ));
    }
    Ok(())
}

fn enforce_depth(depth: usize) -> Result<(), CommandError> {
    if depth > MAX_RECURSIVE_DEPTH {
        Err(CommandError::new(
            "RECURSIVE_DEPTH_LIMIT",
            format!("目录层级超过安全上限 {MAX_RECURSIVE_DEPTH}"),
        ))
    } else {
        Ok(())
    }
}

fn enforce_directory_count(count: u64) -> Result<(), CommandError> {
    if count > MAX_RECURSIVE_DIRECTORIES {
        Err(CommandError::new(
            "RECURSIVE_DIRECTORY_LIMIT",
            format!("目录数量超过安全上限 {MAX_RECURSIVE_DIRECTORIES}"),
        ))
    } else {
        Ok(())
    }
}

fn add_file_to_summary(summary: &mut RecursiveScanSummary, size: u64) -> Result<(), CommandError> {
    let next_count = summary.file_count.saturating_add(1);
    if next_count > MAX_RECURSIVE_FILES {
        return Err(CommandError::new(
            "RECURSIVE_FILE_LIMIT",
            format!("文件数量超过安全上限 {MAX_RECURSIVE_FILES}"),
        ));
    }
    let next_size = summary
        .total_size
        .checked_add(size)
        .ok_or_else(|| CommandError::new("RECURSIVE_SIZE_LIMIT", "目录累计大小超过安全上限"))?;
    if next_size > MAX_RECURSIVE_TOTAL_SIZE {
        return Err(CommandError::new(
            "RECURSIVE_SIZE_LIMIT",
            "目录累计大小超过安全上限 1 TiB",
        ));
    }
    summary.file_count = next_count;
    summary.total_size = next_size;
    Ok(())
}

fn push_warning(summary: &mut RecursiveScanSummary, warning: String) {
    if summary.warnings.len() < MAX_SCAN_WARNINGS {
        summary.warnings.push(warning);
    } else if summary.warnings.len() == MAX_SCAN_WARNINGS {
        summary.warnings.push("更多跳过项未逐条显示".to_owned());
    }
}

fn local_relative_path(root: &Path, path: &Path) -> Result<String, CommandError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))
}

fn normalize_local_boundary(path: &Path) -> String {
    let value = path
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn local_path_within_root(root: &str, path: &str) -> bool {
    path == root || path.starts_with(&format!("{}\\", root.trim_end_matches('\\')))
}

fn normalize_remote_path(path: &str) -> String {
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

fn remote_path_within_root(root: &str, path: &str) -> bool {
    if root == "/" {
        path.starts_with('/')
    } else {
        path == root || path.starts_with(&format!("{}/", root.trim_end_matches('/')))
    }
}

fn join_remote_child(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    }
}

fn is_safe_remote_entry_name(name: &str) -> bool {
    !name.is_empty() && !matches!(name, "." | "..") && !name.contains('/') && !name.contains('\0')
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

fn sftp_error(code: &'static str) -> impl FnOnce(russh_sftp::client::error::Error) -> CommandError {
    move |error| CommandError::new(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_recursive_limits() {
        assert_eq!(
            enforce_depth(MAX_RECURSIVE_DEPTH + 1).unwrap_err().code,
            "RECURSIVE_DEPTH_LIMIT"
        );
        let mut summary = RecursiveScanSummary {
            file_count: MAX_RECURSIVE_FILES,
            ..Default::default()
        };
        assert_eq!(
            add_file_to_summary(&mut summary, 1).unwrap_err().code,
            "RECURSIVE_FILE_LIMIT"
        );
        let mut summary = RecursiveScanSummary {
            total_size: MAX_RECURSIVE_TOTAL_SIZE,
            ..Default::default()
        };
        assert_eq!(
            add_file_to_summary(&mut summary, 1).unwrap_err().code,
            "RECURSIVE_SIZE_LIMIT"
        );
    }

    #[test]
    fn protects_remote_root_boundaries() {
        assert!(remote_path_within_root("/srv/root", "/srv/root/file.txt"));
        assert!(remote_path_within_root("/srv/root", "/srv/root"));
        assert!(!remote_path_within_root(
            "/srv/root",
            "/srv/root-other/file.txt"
        ));
        assert!(!remote_path_within_root("/srv/root", "/etc/passwd"));
    }

    #[tokio::test]
    async fn builds_a_bounded_local_manifest() {
        let root = std::env::temp_dir().join(format!(
            "liteshell-recursive-manifest-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("nested")).await.unwrap();
        fs::write(root.join("root.txt"), b"root").await.unwrap();
        fs::write(root.join("nested").join("child.txt"), b"child")
            .await
            .unwrap();
        let transfers = SftpTransferManager::default();

        let manifest = build_local_directory_manifest(&root, &transfers, "scan-test")
            .await
            .unwrap();
        assert_eq!(manifest.summary.file_count, 2);
        assert_eq!(manifest.summary.directory_count, 1);
        assert_eq!(manifest.summary.total_size, 9);
        assert!(manifest.directories.iter().any(|path| path == "nested"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.relative_path == "nested/child.txt"));

        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn stops_a_cancelled_recursive_scan() {
        let root =
            std::env::temp_dir().join(format!("liteshell-recursive-cancel-{}", std::process::id()));
        fs::create_dir_all(&root).await.unwrap();
        let transfers = SftpTransferManager::default();
        transfers.cancel_operation("scan-cancel").await;

        let error = build_local_directory_manifest(&root, &transfers, "scan-cancel")
            .await
            .unwrap_err();
        assert_eq!(error.code, "RECURSIVE_SCAN_CANCELLED");

        fs::remove_dir_all(root).await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skips_local_symbolic_links() {
        use std::os::unix::fs::symlink;

        let root =
            std::env::temp_dir().join(format!("liteshell-recursive-link-{}", std::process::id()));
        fs::create_dir_all(root.join("real")).await.unwrap();
        symlink(root.join("real"), root.join("linked")).unwrap();
        let transfers = SftpTransferManager::default();

        let manifest = build_local_directory_manifest(&root, &transfers, "scan-link")
            .await
            .unwrap();
        assert_eq!(manifest.summary.skipped_links, 1);
        assert!(!manifest.directories.iter().any(|path| path == "linked"));

        fs::remove_dir_all(root).await.unwrap();
    }
}
