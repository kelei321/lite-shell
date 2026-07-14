from pathlib import Path

path = Path("src-tauri/src/sftp_directory.rs")
text = path.read_text(encoding="utf-8")
old = '''        fs::remove_dir_all(backup).await.map_err(|error| {
            CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
        })?;
'''
new = '''        remove_local_directory_recursive_safe(backup).await.map_err(|error| {
            CommandError::new("LOCAL_DIRECTORY_BACKUP_DELETE_FAILED", error.to_string())
        })?;
'''
if text.count(old) != 1:
    raise SystemExit(f"backup cleanup match count: {text.count(old)}")
text = text.replace(old, new, 1)
marker = '''async fn remove_remote_directory_recursive(
'''
helper = r'''async fn remove_local_directory_recursive_safe(path: &Path) -> Result<(), std::io::Error> {
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

'''
if text.count(marker) != 1:
    raise SystemExit(f"helper marker count: {text.count(marker)}")
text = text.replace(marker, helper + marker, 1)

test_marker = '''    #[tokio::test]
    async fn merge_preserves_existing_local_directory_contents() {
'''
test = r'''    #[cfg(unix)]
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

'''
if text.count(test_marker) != 1:
    raise SystemExit(f"test marker count: {text.count(test_marker)}")
text = text.replace(test_marker, test + test_marker, 1)
path.write_text(text, encoding="utf-8", newline="\n")

Path("scripts/apply_pr9_safe_local_cleanup.py").unlink()
Path(".github/workflows/apply-pr9-safe-cleanup.yml").unlink()
