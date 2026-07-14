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


def patch_app() -> None:
    path = "src/App.vue"
    text = read(path)
    text = replace_once(
        text,
        '''    const existing = state.entries.find((entry) => entry.name === fileName);
    let conflictStrategy: ConflictStrategy = "overwrite";
    if (existing) {
      if (existing.kind !== "file") {
        state.error = `目标“${fileName}”是目录或不支持的条目，不能作为文件覆盖`;
        continue;
      }
      const choice = await chooseFileConflict(fileName, conflicts, paths.length > 1);
      if (choice === "cancel") return;
      conflictStrategy = choice;
      if (conflictStrategy === "skip") continue;
    }
''',
        '''    const inspection = await inspectRemotePath(sessionId, remotePath);
    if (inspection.kind === "directory" || inspection.kind === "symlink" || inspection.kind === "other") {
      state.error = `目标“${fileName}”是目录、链接或不支持的条目，不能作为文件覆盖`;
      continue;
    }
    let conflictStrategy: ConflictStrategy = "overwrite";
    if (inspection.kind === "file") {
      const choice = await chooseFileConflict(fileName, conflicts, paths.length > 1);
      if (choice === "cancel") return;
      conflictStrategy = choice;
      if (conflictStrategy === "skip") continue;
    }
''',
        "direct upload backend inspection",
    )
    text = replace_once(
        text,
        '''    const requestedRoot = joinRemotePath(targetDirectory, manifest.rootName);
    const existing = state.entries.find((entry) => entry.path === requestedRoot || entry.name === manifest.rootName);
    if (existing && existing.kind !== "directory") {
      state.error = `目标“${manifest.rootName}”已存在同名文件或不支持的条目`;
      return;
    }
    let directoryStrategy: DirectoryConflictStrategy = "merge";
    if (existing) {
      const choice = await chooseDirectoryConflict(manifest.rootName, conflicts, allowDirectoryAll);
      if (choice === "cancel" || choice === "skip") return;
      directoryStrategy = choice;
    }
''',
        '''    const requestedRoot = joinRemotePath(targetDirectory, manifest.rootName);
    const rootInspection = await inspectRemotePath(sessionId, requestedRoot);
    if (rootInspection.kind === "file" || rootInspection.kind === "symlink" || rootInspection.kind === "other") {
      state.error = `目标“${manifest.rootName}”已存在同名文件、链接或不支持的条目`;
      return;
    }
    let directoryStrategy: DirectoryConflictStrategy = "merge";
    if (rootInspection.kind === "directory") {
      const choice = await chooseDirectoryConflict(manifest.rootName, conflicts, allowDirectoryAll);
      if (choice === "cancel" || choice === "skip") return;
      directoryStrategy = choice;
    }
''',
        "directory upload backend inspection",
    )
    text = replace_once(
        text,
        '''        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false, sessionId);
          return;
        }
''',
        '''        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false, sessionId);
          prepared = undefined;
          return;
        }
''',
        "cancelled upload replacement cleanup",
    )
    text = replace_once(
        text,
        '''    try {
      await runWithConcurrency(tasks, async (task) => {
        const transferId = crypto.randomUUID();
        const taskId = crypto.randomUUID();
        const request = { taskId, direction: "upload" as const, sessionId, ...task, resume: false };
        transferTasks.set(transferId, request);
        const result = await uploadSftpFile({ sessionId, transferId, taskId, ...task, resume: false });
        if (result.skipped) transferTasks.delete(transferId);
      });
    } catch (error) {
      await rollbackPreparedDirectory(prepared, sessionId, error);
    }
    await finishPreparedDirectory(prepared, true, sessionId);
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
''',
        '''    await runWithConcurrency(tasks, async (task) => {
      const transferId = crypto.randomUUID();
      const taskId = crypto.randomUUID();
      const request = { taskId, direction: "upload" as const, sessionId, ...task, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, transferId, taskId, ...task, resume: false });
      if (result.skipped) transferTasks.delete(transferId);
    });
    await finishPreparedDirectory(prepared, true, sessionId);
    prepared = undefined;
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    if (prepared?.replacementId) {
      try {
        await finishPreparedDirectory(prepared, false, sessionId);
      } catch (rollbackError) {
        state.error = `${describeCommandError(error)}；自动恢复原目录失败：${describeCommandError(rollbackError)}`;
        return;
      }
    }
    state.error = describeCommandError(error);
  }
''',
        "centralized upload replacement rollback",
    )
    write(path, text)


def patch_backend() -> None:
    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
    text = replace_once(
        text,
        '''    if name.is_empty() || matches!(name, "." | "..") {
        return Err(CommandError::new("INVALID_REMOTE_PATH", "远程路径名称无效"));
    }
''',
        '''    if !is_safe_remote_entry_name(name) {
        return Err(CommandError::new("INVALID_REMOTE_PATH", "远程路径名称无效"));
    }
''',
        "safe final remote path name",
    )
    text = replace_once(
        text,
        '''    if path.is_empty() || matches!(path, "/" | "." | "..") || path.contains('\0') {
''',
        '''    if path.is_empty()
        || matches!(path, "/" | "." | "..")
        || path.contains('\0')
        || path.contains('\\')
        || path.split('/').any(|component| component == "..")
    {
''',
        "remote path traversal validation",
    )
    write(path, text)


def patch_docs() -> None:
    handoff = read("handoff.md")
    handoff = replace_once(
        handoff,
        "7. 目录“覆盖”目前实际更接近合并，语义不准确。\n8. 前端和后端各自维护部分传输队列状态，事实来源不统一。\n9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。",
        "7. PR7 已拆分文件与目录冲突语义；仍需本地和远程 replace 实机验证。应用在最终目录切换窗口退出时，可能留下同级 staging/backup 残留，需要人工确认后清理。\n8. 前端和后端各自维护部分传输队列状态，事实来源不统一。\n9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。",
        "handoff PR7 risk",
    )
    write("handoff.md", handoff)

    readme = read("README.md")
    readme = replace_once(
        readme,
        "- PR1～PR6 已完成；PR7 已实现目录冲突分离和事务式替换，正在等待 CI、code review 和合并。\n- 递归删除不是事务操作，中途失败时已经删除的文件无法自动恢复。",
        "- PR1～PR6 已完成；PR7 已实现目录冲突分离和 staging 式替换，正在等待 CI、code review 和合并。\n- 目录替换复制阶段不会改动原目录；若应用恰好在最终 rename 切换窗口退出，可能留下同级 staging/backup 残留，需要人工确认后清理。\n- 递归删除不是事务操作，中途失败时已经删除的文件无法自动恢复。",
        "README replacement limitation",
    )
    write("README.md", readme)

    plan = read("plan.md")
    plan = replace_once(
        plan,
        "本地待测：本地目录替换、远程目录 merge/replace、远程 rename 不支持、替换中断与恢复。",
        "本地待测：本地目录替换、远程目录 merge/replace、远程 rename 不支持、最终 rename 切换中断与 staging/backup 残留恢复。",
        "plan replacement manual validation",
    )
    write("plan.md", plan)


def main() -> None:
    patch_app()
    patch_backend()
    patch_docs()
    Path("scripts/apply_pr9_upload_review_fixes.py").unlink()
    Path(".github/workflows/apply-pr9-upload-review-fixes.yml").unlink()


if __name__ == "__main__":
    main()
