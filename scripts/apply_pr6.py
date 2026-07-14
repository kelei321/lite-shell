from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    Path(path).write_text(content, encoding="utf-8", newline="\n")


def replace_once(content: str, old: str, new: str, label: str) -> str:
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return content.replace(old, new, 1)


def patch_sftp() -> None:
    path = "src-tauri/src/sftp.rs"
    text = read(path)
    text = replace_once(
        text,
        '''#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDirectoryManifest {
    root_name: String,
    directories: Vec<String>,
    files: Vec<LocalManifestFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalManifestFile {
    absolute_path: String,
    relative_path: String,
    size: u64,
}

''',
        '',
        "remove legacy local manifest types",
    )
    text = replace_once(
        text,
        '''        Ok(TransferTaskGuard {
            manager: self,
            task_id: Some(task_id.to_owned()),
        })
    }
}
''',
        '''        Ok(TransferTaskGuard {
            manager: self,
            task_id: Some(task_id.to_owned()),
        })
    }

    pub(crate) async fn begin_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.remove(operation_id);
    }

    pub(crate) async fn cancel_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.insert(operation_id.to_owned());
    }

    pub(crate) async fn operation_cancelled(&self, operation_id: &str) -> bool {
        self.cancelled.lock().await.contains(operation_id)
    }

    pub(crate) async fn finish_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.remove(operation_id);
    }
}
''',
        "add cancellable operation helpers",
    )
    text = replace_once(
        text,
        '''    transfers.cancelled.lock().await.insert(transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn sftp_local_directory_manifest(
    path: String,
) -> Result<LocalDirectoryManifest, CommandError> {
    let root = std::path::PathBuf::from(path.trim());
    if !root.is_dir() {
        return Err(CommandError::new(
            "LOCAL_DIRECTORY_INVALID",
            "本地目录不存在",
        ));
    }
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CommandError::new("LOCAL_DIRECTORY_INVALID", "无法识别本地目录名称"))?
        .to_owned();
    let mut pending = vec![root.clone()];
    let mut directories = Vec::new();
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| CommandError::new("LOCAL_DIRECTORY_READ_FAILED", error.to_string()))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| CommandError::new("LOCAL_ENTRY_READ_FAILED", error.to_string()))?;
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(&root)
                .map_err(|error| CommandError::new("LOCAL_PATH_FAILED", error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            if file_type.is_dir() {
                directories.push(relative);
                pending.push(entry_path);
            } else if file_type.is_file() {
                let size = entry
                    .metadata()
                    .await
                    .map_err(|error| {
                        CommandError::new("LOCAL_FILE_READ_FAILED", error.to_string())
                    })?
                    .len();
                files.push(LocalManifestFile {
                    absolute_path: entry_path.to_string_lossy().into_owned(),
                    relative_path: relative,
                    size,
                });
            }
        }
    }
    directories.sort();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(LocalDirectoryManifest {
        root_name,
        directories,
        files,
    })
}

''',
        '''    transfers.cancel_operation(&transfer_id).await;
    Ok(())
}

''',
        "replace legacy local recursive scan",
    )
    text = replace_once(
        text,
        '''
    #[tokio::test]
    async fn builds_local_directory_manifest() {
        let root = std::env::temp_dir().join(format!("liteshell-sftp-test-{}", std::process::id()));
        fs::create_dir_all(root.join("nested")).await.unwrap();
        fs::write(root.join("root.txt"), b"root").await.unwrap();
        fs::write(root.join("nested").join("child.txt"), b"child")
            .await
            .unwrap();

        let manifest = sftp_local_directory_manifest(root.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert!(manifest.directories.iter().any(|path| path == "nested"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.relative_path == "nested/child.txt"));

        fs::remove_dir_all(root).await.unwrap();
    }
''',
        '\n',
        "remove legacy manifest test",
    )
    write(path, text)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    text = read(path)
    text = replace_once(text, "mod sftp;\n", "mod sftp;\nmod sftp_recursive;\n", "register recursive module")
    text = replace_once(
        text,
        '''    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_local_directory_manifest, sftp_prepare_local_directory,
    sftp_rename, sftp_upload, SftpTransferManager,
};
''',
        '''    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_prepare_local_directory, sftp_rename, sftp_upload,
    SftpTransferManager,
};
use sftp_recursive::{sftp_local_directory_manifest, sftp_remote_directory_manifest};
''',
        "import recursive commands",
    )
    text = replace_once(
        text,
        '''            sftp_local_directory_manifest,
            sftp_prepare_local_directory,
''',
        '''            sftp_local_directory_manifest,
            sftp_remote_directory_manifest,
            sftp_prepare_local_directory,
''',
        "register remote manifest command",
    )
    write(path, text)


def patch_services() -> None:
    path = "src/services/ssh.ts"
    text = read(path)
    text = replace_once(
        text,
        '''export type LocalDirectoryManifest = {
  rootName: string;
  directories: string[];
  files: Array<{ absolutePath: string; relativePath: string; size: number }>;
};
''',
        '''export type RecursiveScanSummary = {
  fileCount: number;
  directoryCount: number;
  totalSize: number;
  skippedLinks: number;
  skippedUnsupported: number;
  warnings: string[];
};

export type LocalDirectoryManifest = RecursiveScanSummary & {
  rootName: string;
  directories: string[];
  files: Array<{ absolutePath: string; relativePath: string; size: number }>;
};

export type RemoteDirectoryManifest = RecursiveScanSummary & {
  rootPath: string;
  directories: string[];
  files: Array<{ remotePath: string; relativePath: string; size: number }>;
};
''',
        "expand manifest types",
    )
    text = replace_once(
        text,
        '''export const getLocalDirectoryManifest = (path: string) =>
  invoke<LocalDirectoryManifest>("sftp_local_directory_manifest", { path });
''',
        '''export const getLocalDirectoryManifest = (path: string, scanId: string) =>
  invoke<LocalDirectoryManifest>("sftp_local_directory_manifest", { path, scanId });

export const getRemoteDirectoryManifest = (sessionId: string, path: string, scanId: string) =>
  invoke<RemoteDirectoryManifest>("sftp_remote_directory_manifest", { sessionId, path, scanId });
''',
        "add bounded manifest service calls",
    )
    write(path, text)


def patch_session_state() -> None:
    path = "src/sftp/session-state.ts"
    text = read(path)
    text = replace_once(text, "  error: string;\n", "  error: string;\n  notice: string;\n", "add session notice type")
    text = replace_once(text, '    error: "",\n', '    error: "",\n    notice: "",\n', "add default notice")
    write(path, text)

    test_path = "src/sftp/session-state.test.mjs"
    test = read(test_path)
    test = replace_once(
        test,
        '''      loading: state.loading,
      historyIndex: state.historyIndex,
''',
        '''      loading: state.loading,
      notice: state.notice,
      historyIndex: state.historyIndex,
''',
        "assert notice state",
    )
    test = replace_once(
        test,
        '''      loading: false,
      historyIndex: -1,
''',
        '''      loading: false,
      notice: "",
      historyIndex: -1,
''',
        "expect notice default",
    )
    write(test_path, test)


def patch_app() -> None:
    path = "src/App.vue"
    text = read(path)
    text = replace_once(
        text,
        '''  getLocalDirectoryManifest,
  disconnectSsh,
''',
        '''  getLocalDirectoryManifest,
  getRemoteDirectoryManifest,
  disconnectSsh,
''',
        "import remote manifest service",
    )
    text = replace_once(
        text,
        '''  type LocalDirectoryManifest,
  type ConnectRequest,
''',
        '''  type LocalDirectoryManifest,
  type RecursiveScanSummary,
  type RemoteDirectoryManifest,
  type ConnectRequest,
''',
        "import recursive manifest types",
    )
    text = replace_once(
        text,
        '''const transfers = ref<TransferEvent[]>([]);
const pendingTransferCheckpoints = ref<TransferCheckpoint[]>([]);
''',
        '''const transfers = ref<TransferEvent[]>([]);
const pendingTransferCheckpoints = ref<TransferCheckpoint[]>([]);
const recursiveScan = ref<{ id: string; sessionId: string; label: string } | null>(null);
''',
        "add active recursive scan",
    )
    text = replace_once(
        text,
        '''async function runWithConcurrency<T>(items: T[], worker: (item: T) => Promise<void>, limit = 3) {
  const queue = [...items];
  const errors: unknown[] = [];
  const workers = Array.from({ length: Math.min(limit, queue.length) }, async () => {
    while (queue.length) {
      const item = queue.shift();
      if (item !== undefined) {
        try {
          await worker(item);
        } catch (error) {
          errors.push(error);
        }
      }
    }
  });
  await Promise.all(workers);
  if (errors.length) throw errors[0];
}
''',
        '''async function runWithConcurrency<T>(items: T[], worker: (item: T) => Promise<void>, limit = 3) {
  const queue = [...items];
  const errors: unknown[] = [];
  const workers = Array.from({ length: Math.min(limit, queue.length) }, async () => {
    while (queue.length) {
      const item = queue.shift();
      if (item !== undefined) {
        try {
          await worker(item);
        } catch (error) {
          errors.push(error);
        }
      }
    }
  });
  await Promise.all(workers);
  if (errors.length) throw errors[0];
}

async function runRecursiveScan<T>(
  sessionId: string,
  label: string,
  scanner: (scanId: string) => Promise<T>,
): Promise<T> {
  if (recursiveScan.value) throw new Error("已有目录扫描正在运行");
  const scanId = crypto.randomUUID();
  recursiveScan.value = { id: scanId, sessionId, label };
  try {
    return await scanner(scanId);
  } finally {
    if (recursiveScan.value?.id === scanId) recursiveScan.value = null;
  }
}

function updateRecursiveScanNotice(state: SftpSessionState, summary: RecursiveScanSummary) {
  const skipped = summary.skippedLinks + summary.skippedUnsupported;
  state.notice = skipped
    ? `目录扫描完成：${summary.fileCount} 个文件、${summary.directoryCount} 个目录，已跳过 ${summary.skippedLinks} 个链接和 ${summary.skippedUnsupported} 个不支持项。${summary.warnings[0] ?? ""}`
    : `目录扫描完成：${summary.fileCount} 个文件、${summary.directoryCount} 个目录，共 ${formatBytes(summary.totalSize)}。`;
}

function scanLocalDirectory(path: string, sessionId: string) {
  return runRecursiveScan(sessionId, "正在安全扫描本地目录", (scanId) => getLocalDirectoryManifest(path, scanId));
}

function scanRemoteDirectory(sessionId: string, path: string): Promise<RemoteDirectoryManifest> {
  return runRecursiveScan(sessionId, "正在安全扫描远程目录", (scanId) => getRemoteDirectoryManifest(sessionId, path, scanId));
}

async function cancelRecursiveScan() {
  const scan = recursiveScan.value;
  if (!scan) return;
  await cancelSftpTransfer(scan.id).catch((error) => {
    ensureSftpSessionState(sftpStates, scan.sessionId).error = describeCommandError(error);
  });
}
''',
        "add recursive scan helpers",
    )
    text = replace_once(
        text,
        '''    const manifest = knownManifest ?? await getLocalDirectoryManifest(localPath);
    let remoteRoot = joinRemotePath(targetDirectory, manifest.rootName);
''',
        '''    const manifest = knownManifest ?? await scanLocalDirectory(localPath, sessionId);
    updateRecursiveScanNotice(state, manifest);
    let remoteRoot = joinRemotePath(targetDirectory, manifest.rootName);
''',
        "use bounded local manifest",
    )
    start = text.index("async function startDownload() {")
    end = text.index("async function downloadOne(", start)
    new_download = '''async function startDownload() {
  const sessionId = activeSessionId.value;
  const state = sftpStates.get(sessionId);
  const selected = state ? [...state.selectedEntries] : [];
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!state || !session?.connected || !selectionBelongsToSession(selected, sessionId)) {
    if (state) state.error = "请先选择要下载的文件或文件夹";
    return;
  }
  if (selected.length === 1 && selected[0].kind === "file") {
    const item = selected[0];
    const localPath = await save({ title: "保存远程文件", defaultPath: item.name });
    if (!localPath) return;
    try {
      await downloadOne(sessionId, item.path, localPath, "overwrite");
    } catch (error) {
      state.error = describeCommandError(error);
    }
    return;
  }
  if (selected.length === 1 && selected[0].kind !== "directory") {
    state.notice = "符号链接和不支持的远程条目不会被递归下载";
    return;
  }
  const localRoot = await open({ directory: true, multiple: false, title: "选择下载保存目录" });
  if (!localRoot || Array.isArray(localRoot)) return;
  const conflict = await requestConflict("批量下载中的同名文件", true);
  if (conflict.strategy === "cancel") return;
  const batchConflictStrategy = conflict.strategy;
  try {
    const downloads: Array<{ remotePath: string; localPath: string }> = [];
    let skippedDirectEntries = 0;
    for (const item of selected) {
      if (item.kind === "file") {
        downloads.push({ remotePath: item.path, localPath: joinLocalPath(localRoot, item.name) });
        continue;
      }
      if (item.kind !== "directory") {
        skippedDirectEntries += 1;
        continue;
      }
      const preparedRoot = await prepareLocalDirectory(joinLocalPath(localRoot, item.name), batchConflictStrategy);
      if (preparedRoot.skipped) continue;
      const manifest = await scanRemoteDirectory(sessionId, item.path);
      updateRecursiveScanNotice(state, manifest);
      for (const directory of manifest.directories) {
        await prepareLocalDirectory(joinLocalPath(preparedRoot.path, directory), "overwrite");
      }
      for (const file of manifest.files) {
        downloads.push({
          remotePath: file.remotePath,
          localPath: joinLocalPath(preparedRoot.path, file.relativePath),
        });
      }
    }
    if (skippedDirectEntries) {
      state.notice = `${state.notice} 已跳过 ${skippedDirectEntries} 个直接选择的链接或不支持项。`.trim();
    }
    await runWithConcurrency(downloads, (item) => downloadOne(sessionId, item.remotePath, item.localPath, batchConflictStrategy));
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

'''
    text = text[:start] + new_download + text[end:]
    text = replace_once(
        text,
        '''      const manifest = await getLocalDirectoryManifest(path);
      await uploadDirectoryPath(path, manifest, sessionId);
''',
        '''      const manifest = await scanLocalDirectory(path, sessionId);
      await uploadDirectoryPath(path, manifest, sessionId);
''',
        "scan dropped directories safely",
    )
    text = replace_once(
        text,
        '''          <div v-if="pendingTransferCheckpoints.length" class="transfer-queue checkpoint-queue">
''',
        '''          <div v-if="recursiveScan && recursiveScan.sessionId === activeSessionId" class="sftp-scan-status"><span>{{ recursiveScan.label }}…</span><button @click="cancelRecursiveScan">取消扫描</button></div>
          <div v-if="pendingTransferCheckpoints.length" class="transfer-queue checkpoint-queue">
''',
        "show cancellable scan status",
    )
    text = replace_once(
        text,
        '''          <div v-if="sftpError" class="sftp-error">{{ sftpError }}</div>
''',
        '''          <div v-if="activeSftpState.notice" class="sftp-notice">{{ activeSftpState.notice }}</div>
          <div v-if="sftpError" class="sftp-error">{{ sftpError }}</div>
''',
        "show scan notice",
    )
    write(path, text)


def patch_styles() -> None:
    path = "src/styles.css"
    text = read(path)
    text = replace_once(
        text,
        '''.sftp-error { padding: 7px 12px; color: #ffabb1; background: #38232a; border-bottom: 1px solid #65404a; font-size: 11px; }
''',
        '''.sftp-scan-status, .sftp-notice { padding: 7px 12px; border-bottom: 1px solid #345364; background: #172f3b; color: #9fd6f1; font-size: 11px; }
.sftp-scan-status { display: flex; align-items: center; justify-content: space-between; }
.sftp-scan-status button { border: 0; background: transparent; color: #79bfe8; cursor: pointer; }
.sftp-error { padding: 7px 12px; color: #ffabb1; background: #38232a; border-bottom: 1px solid #65404a; font-size: 11px; }
''',
        "add recursive scan styles",
    )
    write(path, text)


def patch_docs() -> None:
    plan_path = "plan.md"
    plan = read(plan_path)
    plan = replace_once(
        plan,
        "状态：实施进行中，PR1～PR4 已完成，PR5 已实现并待 CI/合并验证",
        "状态：实施进行中，PR1～PR5 已完成，PR6 已实现并待 CI/合并验证",
        "plan overall status",
    )
    plan = replace_once(
        plan,
        "| PR5 | 安全断点续传和任务检查点 | `feat/sftp-safe-resume-checkpoint` | 待验证 | PR4 |\n| PR6 | 递归传输和符号链接安全 | `fix/sftp-recursive-transfer-safety` | 待开始 | PR5 |",
        "| PR5 | 安全断点续传和任务检查点 | `feat/sftp-safe-resume-checkpoint` | 已完成 | PR4 |\n| PR6 | 递归传输和符号链接安全 | `fix/sftp-recursive-transfer-safety` | 待验证 | PR5 |",
        "plan status table",
    )
    plan = replace_once(plan, "## 10. PR6：递归传输和符号链接安全\n\n状态：`待开始`", "## 10. PR6：递归传输和符号链接安全\n\n状态：`待验证`", "plan PR6 status")
    plan = replace_once(
        plan,
        "### 完成记录\n\n尚未开始。\n\n---\n\n## 11. PR7：明确目录冲突语义",
        """### 完成记录

实现内容：

- 新增 Rust 端本地和远程递归 manifest 扫描，前端不再自行递归远程列表。
- 默认跳过本地符号链接、Windows reparse point/junction、远程符号链接和不支持条目。
- 本地使用 canonical path、visited 集合和根边界校验；远程使用 canonical path、受控子路径拼接、visited 集合和根边界校验。
- 最大深度 64、文件数 100000、目录数 100000、累计大小 1 TiB，超限返回稳定错误。
- 扫描复用取消标记，UI 显示扫描状态并支持取消。
- manifest 返回文件数、目录数、总大小、跳过链接数、跳过不支持项和 warnings，UI 明确展示跳过汇总。
- 新增边界、限制、取消、本地 manifest 和 Unix 符号链接测试；Windows junction 仍需本地实机验证。

验证：等待 GitHub Actions；未执行真实服务器写入测试。

本地待测：Windows junction 循环、远程 symlink 指向根外、深层目录、超大量小文件和扫描取消。

下一步：PR7：明确目录冲突语义。

---

## 11. PR7：明确目录冲突语义""",
        "plan PR6 completion",
    )
    write(plan_path, plan)

    readme_path = "README.md"
    readme = read(readme_path)
    readme = replace_once(
        readme,
        "- 断点续传使用稳定任务 ID、后端验证的 SSH 服务器身份和持久检查点，源身份不匹配时拒绝拼接。",
        "- 断点续传使用稳定任务 ID、后端验证的 SSH 服务器身份和持久检查点，源身份不匹配时拒绝拼接。\n- 递归上传下载由 Rust 安全扫描 manifest，默认跳过符号链接/junction，并限制深度、数量和累计大小。",
        "README recursive capability",
    )
    readme = replace_once(
        readme,
        "5. 安全断点续传和任务检查点（PR5 已实现，等待 CI 和合并）。\n6. 递归传输和符号链接安全。",
        "5. 安全断点续传和任务检查点（PR5 已完成）。\n6. 递归传输和符号链接安全（PR6 已实现，等待 CI 和合并）。",
        "README roadmap",
    )
    readme = replace_once(
        readme,
        "- PR1～PR4 已完成；PR5 已实现源身份校验和持久检查点，递归与符号链接安全仍需按 `plan.md` 的 PR6 完成。",
        "- PR1～PR5 已完成；PR6 已实现受限递归扫描和链接跳过，目录冲突语义仍需按 `plan.md` 的 PR7 完成。",
        "README current limit",
    )
    write(readme_path, readme)

    handoff_path = "handoff.md"
    handoff = read(handoff_path)
    handoff = replace_once(
        handoff,
        "当前状态：PR1～PR4 已合并；PR5 安全断点续传和任务检查点已实现，正在等待 CI、code review 和合并。PR5 合并后进入 PR6。",
        "当前状态：PR1～PR5 已合并；PR6 递归传输和符号链接安全已实现，正在等待 CI、code review 和合并。PR6 合并后进入 PR7。",
        "handoff current state",
    )
    handoff = replace_once(
        handoff,
        "- PR5：稳定 taskId、后端验证的 SSH 身份、应用数据目录检查点和内容采样指纹，拒绝不安全续传。",
        "- PR5：稳定 taskId、后端验证的 SSH 身份、应用数据目录检查点和内容采样指纹，拒绝不安全续传。\n- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总。",
        "handoff PR6 capability",
    )
    handoff = replace_once(
        handoff,
        "6. 递归下载当前会把 symlink 当作可下载文件处理。\n7. 本地递归扫描需要补 junction、深度、数量和取消保护。\n8. 目录“覆盖”目前实际更接近合并，语义不准确。",
        "6. PR6 已处理递归 symlink/junction、根边界、深度、数量、累计大小和取消；仍需 Windows 与真实服务器实机验证。\n7. 目录“覆盖”目前实际更接近合并，语义不准确。",
        "handoff remaining risks",
    )
    write(handoff_path, handoff)


def main() -> None:
    patch_sftp()
    patch_lib()
    patch_services()
    patch_session_state()
    patch_app()
    patch_styles()
    patch_docs()
    Path("scripts/apply_pr6.py").unlink()
    Path(".github/workflows/apply-pr6.yml").unlink()


if __name__ == "__main__":
    main()
