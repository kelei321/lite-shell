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


def replace_section(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    end_index = text.find(end, start_index + len(start))
    if start_index < 0 or end_index < 0:
        raise RuntimeError(f"{label}: markers not found")
    return text[:start_index] + replacement.rstrip() + "\n\n" + text[end_index:]


def patch_backend() -> None:
    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
    text = replace_once(
        text,
        '''enum DirectoryReplacement {
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
''',
        '''enum DirectoryReplacement {
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
    let value = path.to_string_lossy().replace('\\\\', "/");
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value.into_owned()
    }
}
''',
        "staging replacement model",
    )
    text = replace_once(
        text,
        '''        if transactions.contains_key(replacement_id) {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        transactions.insert(replacement_id.to_owned(), replacement);
''',
        '''        if transactions.contains_key(replacement_id) {
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
''',
        "same target lock",
    )
    text = replace_once(
        text,
        '''        DirectoryReplacement::Local { target, backup } => {
            finish_local_replacement(&target, &backup, commit).await
        }
        DirectoryReplacement::Remote {
            server_id,
            target,
            backup,
        } => {
''',
        '''        DirectoryReplacement::Local {
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
''',
        "finish replacement fields",
    )
    text = replace_once(
        text,
        '''            let result = finish_remote_replacement(&sftp, &target, &backup, commit).await;
''',
        '''            let result =
                finish_remote_replacement(&sftp, &target, &staging, &backup, commit).await;
''',
        "remote finish staging",
    )
    local_start = text.find("        DirectoryConflictStrategy::Replace => {", text.find("async fn prepare_local_directory"))
    local_end = text.find("\n        }\n    }\n}\n\nasync fn prepare_remote_directory", local_start)
    if local_start < 0 or local_end < 0:
        raise RuntimeError("local replace block not found")
    local_block = '''        DirectoryConflictStrategy::Replace => {
            let replacement_id = replacement_id.ok_or_else(|| {
                CommandError::new("INVALID_DIRECTORY_REPLACEMENT", "目录替换标识不能为空")
            })?;
            replacements.ensure_available(replacement_id)?;
            let staging = local_staging_path(&target, replacement_id)?;
            let backup = local_backup_path(&target, replacement_id)?;
            if fs::symlink_metadata(&staging).await.is_ok()
                || fs::symlink_metadata(&backup).await.is_ok()
            {
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
        }'''
    text = text[:local_start] + local_block + text[local_end + len("\n        }"):]

    remote_start = text.find("        DirectoryConflictStrategy::Replace => {", text.find("async fn prepare_remote_directory"))
    remote_end = text.find("\n        }\n    }\n}\n\nasync fn finish_local_replacement", remote_start)
    if remote_start < 0 or remote_end < 0:
        raise RuntimeError("remote replace block not found")
    remote_block = '''        DirectoryConflictStrategy::Replace => {
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
        }'''
    text = text[:remote_start] + remote_block + text[remote_end + len("\n        }"):]

    text = replace_section(
        text,
        "async fn finish_local_replacement(",
        "async fn remove_remote_directory_recursive(",
        '''async fn finish_local_replacement(
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
    if local_path_exists(target).await? && !local_path_exists(staging).await? {
        if local_path_exists(backup).await? {
            fs::remove_dir_all(backup).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
            })?;
        }
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
    {
        if remote_path_kind(sftp, backup).await? == "directory" {
            remove_remote_directory_recursive(sftp, backup).await?;
        }
        return Ok(());
    }
    Err(CommandError::new(
        "SFTP_DIRECTORY_REPLACEMENT_INVALID_STATE",
        "远程目录替换状态不完整，已停止提交",
    ))
}''',
        "staged replacement finish",
    )
    text = replace_section(
        text,
        "async fn remove_remote_directory_recursive(",
        "async fn remote_path_kind(",
        '''async fn remove_remote_directory_recursive(
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
            .read_dir(current.clone())
            .await
            .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_LIST_FAILED"))?;
        for entry in entries {
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
                stack.push((canonical, false));
            } else {
                sftp.remove_file(child)
                    .await
                    .map_err(sftp_error("SFTP_DIRECTORY_REPLACE_DELETE_FAILED"))?;
            }
        }
    }
    Ok(())
}''',
        "safe remote replacement cleanup",
    )
    text = replace_once(
        text,
        '''fn local_backup_path(target: &Path, replacement_id: &str) -> Result<PathBuf, CommandError> {
''',
        '''async fn local_path_exists(path: &Path) -> Result<bool, CommandError> {
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
    Ok(target.with_file_name(format!(
        "{name}.liteshell-dir-staging-{replacement_id}"
    )))
}

fn local_backup_path(target: &Path, replacement_id: &str) -> Result<PathBuf, CommandError> {
''',
        "local staging helpers",
    )
    text = replace_once(
        text,
        '''fn remote_backup_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
''',
        '''fn remote_staging_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
    validate_remote_directory_path(path)?;
    Ok(format!(
        "{}.liteshell-dir-staging-{replacement_id}",
        path.trim_end_matches('/')
    ))
}

fn remote_backup_path(path: &str, replacement_id: &str) -> Result<String, CommandError> {
''',
        "remote staging helper",
    )
    text = replace_once(
        text,
        '''fn validate_replacement_id(replacement_id: &str) -> Result<(), CommandError> {
''',
        '''fn is_safe_remote_entry_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && !name.contains('/')
        && !name.contains('\\\\')
        && !name.contains('\\0')
}

fn join_remote_child(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    }
}

fn validate_replacement_id(replacement_id: &str) -> Result<(), CommandError> {
''',
        "safe remote path helpers",
    )
    text = replace_section(
        text,
        "    #[tokio::test]\n    async fn rollback_restores_the_original_local_directory()",
        "}\n",
        '''    #[tokio::test]
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
        fs::write(staging_path.join("new.txt"), b"new").await.unwrap();
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
}''',
        "staged replacement tests",
    )
    write(path, text)


def patch_frontend() -> None:
    path = "src/App.vue"
    text = read(path)
    text = replace_once(
        text,
        "localPath.split(/[\\/]/).pop()",
        "localPath.split(/[\\\\/]/).pop()",
        "Windows filename separator",
    )
    text = replace_once(
        text,
        "原目录会先安全备份，复制失败时自动恢复。确定继续吗？",
        "新内容会先写入独立临时目录，提交时再安全备份并替换原目录；复制失败不会改动原目录。确定继续吗？",
        "replace confirmation",
    )
    text = replace_once(
        text,
        "合并会保留目标中的额外内容；替换会先备份原目录，并删除目标中源目录不存在的额外内容。",
        "合并会保留目标中的额外内容；替换会先写入独立临时目录，提交后删除目标中源目录不存在的额外内容。",
        "replace explanation",
    )
    write(path, text)


def patch_docs() -> None:
    for path in ["plan.md", "handoff.md"]:
        text = read(path)
        text = text.replace(
            "目录替换使用不透明 replacement ID 绑定目标、备份和服务器身份。",
            "目录替换使用不透明 replacement ID 绑定目标、staging、备份和服务器身份。",
        )
        text = text.replace(
            "替换前先将原目录安全 rename 到备份；复制失败自动删除新目录并恢复原目录。",
            "替换内容先写入独立 staging 目录；复制失败只删除 staging，原目录保持不变。提交时再执行原目录备份和 staging 切换。",
        )
        text = text.replace(
            "本地和远程目录替换先安全备份，复制失败时回滚恢复，成功后提交清理备份。",
            "本地和远程目录替换先写入独立 staging；复制失败不改动原目录，提交时才备份并切换，成功后清理备份。",
        )
        write(path, text)

    readme = read("README.md")
    readme = readme.replace(
        "目录冲突独立支持合并、跳过、重命名和事务式替换。",
        "目录冲突独立支持合并、跳过、重命名和 staging 式安全替换。",
    )
    write("README.md", readme)

    gitignore = read(".gitignore")
    if "__pycache__/" not in gitignore:
        gitignore = gitignore.replace(
            "# Local development files\n",
            "# Local development files\n__pycache__/\n*.py[cod]\n",
            1,
        )
    write(".gitignore", gitignore)


def main() -> None:
    patch_backend()
    patch_frontend()
    patch_docs()
    Path("scripts/apply_pr9_review_fixes.py").unlink()
    Path(".github/workflows/apply-pr9-review-fixes.yml").unlink()


if __name__ == "__main__":
    main()
