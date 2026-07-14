from pathlib import Path


path = Path("scripts/apply_pr9_review_fixes.py")
text = path.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    '''def replace_section(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    end_index = text.find(end, start_index + len(start))
    if start_index < 0 or end_index < 0:
        raise RuntimeError(f"{label}: markers not found")
    return text[:start_index] + replacement.rstrip() + "\\n\\n" + text[end_index:]
''',
    '''def replace_section(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if label == "staged replacement tests":
        if start_index < 0 or text.rfind("\\n}") < start_index:
            raise RuntimeError(f"{label}: module markers not found")
        return text[:start_index] + replacement.rstrip() + "\\n"
    end_index = text.find(end, start_index + len(start))
    if start_index < 0 or end_index < 0:
        raise RuntimeError(f"{label}: markers not found")
    return text[:start_index] + replacement.rstrip() + "\\n\\n" + text[end_index:]
''',
    "test module replacement",
)
replace_once(
    '''    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
''',
    '''    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
    text = replace_once(
        text,
        "    collections::HashMap,",
        "    collections::{HashMap, HashSet},",
        "replacement cleanup visited import",
    )
''',
    "HashSet import patch",
)
replace_once(
    '''    #[cfg(not(windows))]
    {
        value.into_owned()
    }
''',
    '''    #[cfg(not(windows))]
    {
        value
    }
''',
    "non-Windows target normalization",
)
replace_once(
    '''            if fs::symlink_metadata(&staging).await.is_ok()
                || fs::symlink_metadata(&backup).await.is_ok()
            {
''',
    '''            if local_path_exists(&staging).await? || local_path_exists(&backup).await? {
''',
    "local replacement path inspection",
)
replace_once(
    '''    if local_path_exists(target).await? && !local_path_exists(staging).await? {
        if local_path_exists(backup).await? {
            fs::remove_dir_all(backup).await.map_err(|error| {
                CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
            })?;
        }
        return Ok(());
    }
''',
    '''    if local_path_exists(target).await?
        && !local_path_exists(staging).await?
        && local_path_exists(backup).await?
    {
        fs::remove_dir_all(backup).await.map_err(|error| {
            CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
        })?;
        return Ok(());
    }
''',
    "local commit completion state",
)
replace_once(
    '''    if remote_path_kind(sftp, target).await? == "directory"
        && remote_path_kind(sftp, staging).await? == "missing"
    {
        if remote_path_kind(sftp, backup).await? == "directory" {
            remove_remote_directory_recursive(sftp, backup).await?;
        }
        return Ok(());
    }
''',
    '''    if remote_path_kind(sftp, target).await? == "directory"
        && remote_path_kind(sftp, staging).await? == "missing"
        && remote_path_kind(sftp, backup).await? == "directory"
    {
        remove_remote_directory_recursive(sftp, backup).await?;
        return Ok(());
    }
''',
    "remote commit completion state",
)
replace_once(
    '''    let root_prefix = format!("{}/", root.trim_end_matches('/'));
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
''',
    '''    let root_prefix = format!("{}/", root.trim_end_matches('/'));
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
''',
    "bounded remote replacement cleanup",
)

path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/fix_apply_pr9_review_script.py").unlink()
