from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    Path(path).write_text(content, encoding="utf-8", newline="\n")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def patch_directory_module() -> None:
    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
    text = replace_once(
        text,
        '''pub struct LocalPathInspection {
    kind: &'static str,
}
''',
        '''pub struct LocalPathInspection {
    kind: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePathInspection {
    kind: &'static str,
}
''',
        "remote inspection type",
    )
    text = replace_once(
        text,
        '''#[tauri::command]
pub async fn sftp_prepare_local_directory(
''',
        '''#[tauri::command]
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
''',
        "remote inspection command",
    )
    text = replace_once(
        text,
        '''    let existing = remote_metadata(sftp, path).await?;
    if let Some(metadata) = &existing {
        if !metadata.is_dir() {
            return Err(CommandError::new(
                "SFTP_TARGET_IS_FILE",
                "目标路径已存在同名文件",
            ));
        }
    }
    let existed = existing.is_some();
''',
        '''    let existing_kind = remote_path_kind(sftp, path).await?;
    if !matches!(existing_kind, "missing" | "directory") {
        return Err(CommandError::new(
            "SFTP_TARGET_IS_NOT_DIRECTORY",
            "目标路径已存在同名文件、符号链接或不支持的条目",
        ));
    }
    let existed = existing_kind == "directory";
''',
        "safe remote existing kind",
    )
    text = replace_once(
        text,
        '''            if remote_metadata(sftp, &backup).await?.is_some() {
''',
        '''            if remote_path_kind(sftp, &backup).await? != "missing" {
''',
        "safe remote backup kind",
    )
    text = replace_once(
        text,
        '''        if remote_metadata(sftp, &candidate).await?.is_none() {
            return Ok(candidate);
        }
''',
        '''        if remote_path_kind(sftp, &candidate).await? == "missing" {
            return Ok(candidate);
        }
''',
        "safe remote unique kind",
    )
    start = text.find("async fn remote_metadata(")
    end = text.find("async fn unique_local_directory_path(", start)
    if start < 0 or end < 0:
        raise RuntimeError("remote metadata section not found")
    replacement = '''async fn remote_path_kind(
    sftp: &SftpSession,
    path: &str,
) -> Result<&'static str, CommandError> {
    let trimmed = path.trim_end_matches('/');
    let (parent, name) = match trimmed.rsplit_once('/') {
        Some(("", name)) => ("/", name),
        Some((parent, name)) => (parent, name),
        None => (".", trimmed),
    };
    if name.is_empty() || matches!(name, "." | "..") {
        return Err(CommandError::new(
            "INVALID_REMOTE_PATH",
            "远程路径名称无效",
        ));
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

'''
    text = text[:start] + replacement + text[end:]
    write(path, text)


def patch_apply_script() -> None:
    path = "scripts/apply_pr7.py"
    text = read(path)
    for old, new, label in [
        ("\nfunction sftpStorageScope(sessionId: string)'''", "'''", "duplicate storage marker"),
        ("\nasync function startUploadDirectory()'''", "'''", "duplicate upload marker"),
        ("\nasync function startDownload()'''", "'''", "duplicate download marker"),
        ("\nasync function downloadOne('''", "'''", "duplicate downloadOne marker"),
        ("\nfunction formatUptime(seconds = 0)'''", "'''", "duplicate uptime marker"),
    ]:
        text = replace_once(text, old, new, label)
    text = replace_once(
        text,
        '''    text = replace_once(text, "  type LocalDirectoryManifest,\\n", "  type LocalDirectoryManifest,\\n  type LocalPathKind,\\n", "local path type import")
''',
        "",
        "remove unused local path import",
    )
    text = replace_once(
        text,
        '''    text = replace_once(text, "  isTauri,\\n", "  inspectLocalPath,\\n  isTauri,\\n", "inspect import")
''',
        '''    text = replace_once(text, "  isTauri,\\n", "  inspectLocalPath,\\n  inspectRemotePath,\\n  isTauri,\\n", "inspect imports")
''',
        "remote inspect import",
    )
    text = replace_once(
        text,
        '''    text = replace_once(
        text,
        ''' + "'''" + '''            sftp_inspect_local_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
        ''' + "'''" + '''            sftp_inspect_local_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
        "noop",
    )
''',
        "",
        "remove nonexistent noop",
    ) if "noop" in text else text
    text = replace_once(
        text,
        '''        ''' + "'''" + '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
        ''' + "'''" + '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_inspect_remote_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
''',
        '''        ''' + "'''" + '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
        ''' + "'''" + '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_inspect_remote_path,
            sftp_prepare_local_directory,
''' + "'''" + ''',
''',
        "lib remote inspect command",
    )
    text = replace_once(
        text,
        '''export const inspectLocalPath = (path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_local_path", { path });

export const prepareLocalDirectory = (
''',
        '''export const inspectLocalPath = (path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_local_path", { path });

export const inspectRemotePath = (sessionId: string, path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_remote_path", { sessionId, path });

export const prepareLocalDirectory = (
''',
        "service remote inspect",
    )
    text = replace_once(
        text,
        '''    const existingFiles = new Set<string>();
    if (prepared.existed && directoryStrategy === "merge") {
      const existingManifest = await scanRemoteDirectory(sessionId, prepared.path);
      for (const file of existingManifest.files) existingFiles.add(file.relativePath);
    }
    for (const directory of manifest.directories) {
      await createSftpDirectory(sessionId, joinRemotePath(prepared.path, directory));
    }
''',
        '''    for (const directory of manifest.directories) {
      await prepareRemoteDirectory(sessionId, joinRemotePath(prepared.path, directory), "merge");
    }
''',
        "safe remote nested directories",
    )
    text = replace_once(
        text,
        '''      let conflictStrategy: ConflictStrategy = "overwrite";
      if (existingFiles.has(file.relativePath)) {
        const choice = await chooseFileConflict(file.relativePath, conflicts, true);
        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false, sessionId);
          return;
        }
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      tasks.push({
        localPath: file.absolutePath,
        remotePath: joinRemotePath(prepared.path, file.relativePath),
        conflictStrategy,
      });
''',
        '''      const remotePath = joinRemotePath(prepared.path, file.relativePath);
      const inspection = await inspectRemotePath(sessionId, remotePath);
      if (inspection.kind === "directory" || inspection.kind === "symlink" || inspection.kind === "other") {
        throw new Error(`远程目标“${file.relativePath}”已存在同名目录、链接或不支持的条目`);
      }
      let conflictStrategy: ConflictStrategy = "overwrite";
      if (inspection.kind === "file") {
        const choice = await chooseFileConflict(file.relativePath, conflicts, true);
        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false, sessionId);
          return;
        }
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      tasks.push({ localPath: file.absolutePath, remotePath, conflictStrategy });
''',
        "safe remote file targets",
    )
    text = replace_once(
        text,
        '''    const existingFiles = new Set<string>();
    if (prepared.existed && directoryStrategy === "merge") {
      const localManifest = await scanLocalDirectory(prepared.path, sessionId);
      for (const file of localManifest.files) existingFiles.add(file.relativePath);
    }
''',
        "",
        "remove local manifest conflict detection",
    )
    text = replace_once(
        text,
        '''      let conflictStrategy: ConflictStrategy = "overwrite";
      if (existingFiles.has(file.relativePath)) {
        const choice = await chooseFileConflict(file.relativePath, conflicts, true);
        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false);
          return false;
        }
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      downloads.push({
        remotePath: file.remotePath,
        localPath: joinLocalPath(prepared.path, file.relativePath),
        conflictStrategy,
      });
''',
        '''      const localPath = joinLocalPath(prepared.path, file.relativePath);
      const inspection = await inspectLocalPath(localPath);
      if (inspection.kind === "directory" || inspection.kind === "other") {
        throw new Error(`本地目标“${file.relativePath}”已存在同名目录、链接或不支持的条目`);
      }
      let conflictStrategy: ConflictStrategy = "overwrite";
      if (inspection.kind === "file") {
        const choice = await chooseFileConflict(file.relativePath, conflicts, true);
        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false);
          return false;
        }
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      downloads.push({ remotePath: file.remotePath, localPath, conflictStrategy });
''',
        "safe local file targets",
    )
    text = replace_once(
        text,
        '''    try {
      await runWithConcurrency(downloads, (download) => downloadOne(
        sessionId,
        download.remotePath,
        download.localPath,
        download.conflictStrategy,
      ));
    } catch (error) {
      await rollbackPreparedDirectory(prepared, undefined, error);
    }
    await finishPreparedDirectory(prepared, true);
''',
        '''    await runWithConcurrency(downloads, (download) => downloadOne(
      sessionId,
      download.remotePath,
      download.localPath,
      download.conflictStrategy,
    ));
    await finishPreparedDirectory(prepared, true);
''',
        "single download rollback",
    )
    text = replace_once(
        text,
        '"状态：实施进行中，PR1～PR5 已完成，PR6 已实现并待验证，下一步为 PR7",',
        '"状态：实施进行中，PR1～PR5 已完成，PR6 已实现并待 CI/合并验证",',
        "plan current status source",
    )
    text = replace_once(
        text,
        '''        "- 递归上传和下载默认跳过本地 symlink、Windows junction/reparse point 与远程 symlink，并限制深度、数量和累计大小。",
        "- 递归上传和下载默认跳过本地 symlink、Windows junction/reparse point 与远程 symlink，并限制深度、数量和累计大小。\\n- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和事务式替换。",
''',
        '''        "- 递归上传下载由 Rust 安全扫描 manifest，默认跳过符号链接/junction，并限制深度、数量和累计大小。",
        "- 递归上传下载由 Rust 安全扫描 manifest，默认跳过符号链接/junction，并限制深度、数量和累计大小。\\n- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和事务式替换。",
''',
        "README feature source",
    )
    text = replace_once(
        text,
        '''        "- PR1～PR6 已完成；下一步按 `plan.md` 处理 PR7 目录冲突语义。",
        "- PR1～PR7 已实现；PR7 正在等待 CI、code review 和合并，下一步按 `plan.md` 处理 PR8 后端统一传输队列。",
''',
        '''        "- PR1～PR5 已完成；PR6 已实现受限递归扫描和链接跳过，目录冲突语义仍需按 `plan.md` 的 PR7 完成。",
        "- PR1～PR6 已完成；PR7 已实现目录冲突分离和事务式替换，正在等待 CI、code review 和合并。",
''',
        "README progress source",
    )
    text = replace_once(
        text,
        '''    readme = replace_once(
        readme,
        "- PR1～PR5 已完成；PR6 已实现受限递归扫描和链接跳过，目录冲突语义仍需按 `plan.md` 的 PR7 完成。",
        "- PR1～PR6 已完成；PR7 已实现目录冲突分离和事务式替换，正在等待 CI、code review 和合并。",
        "README progress",
    )
''',
        '''    readme = replace_once(
        readme,
        "6. 递归传输和符号链接安全（PR6 已实现，等待 CI 和合并）。\\n7. 明确目录冲突语义。",
        "6. 递归传输和符号链接安全（PR6 已完成）。\\n7. 明确目录冲突语义（PR7 已实现，等待 CI 和合并）。",
        "README roadmap progress",
    )
    readme = replace_once(
        readme,
        "- PR1～PR5 已完成；PR6 已实现受限递归扫描和链接跳过，目录冲突语义仍需按 `plan.md` 的 PR7 完成。",
        "- PR1～PR6 已完成；PR7 已实现目录冲突分离和事务式替换，正在等待 CI、code review 和合并。",
        "README progress",
    )
''',
        "README dual progress",
    )
    text = text.replace(
        "PR7 合并后下一任务为 PR8：后端统一传输队列、暂停和恢复，分支建议 `feat/sftp-transfer-queue`.",
        "PR7 合并后下一任务为 PR8：后端统一传输队列、暂停和恢复，分支建议 `feat/sftp-transfer-queue`。",
    )
    write(path, text)


def main() -> None:
    patch_directory_module()
    patch_apply_script()
    Path("scripts/fix_apply_pr7.py").unlink()


if __name__ == "__main__":
    main()
