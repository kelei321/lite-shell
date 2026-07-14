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


# Ensure early cancellation is not cleared and remote open failures still clean scan state.
sftp_path = "src-tauri/src/sftp.rs"
sftp = read(sftp_path)
sftp = replace_once(
    sftp,
    '''    pub(crate) async fn begin_operation(&self, operation_id: &str) {
        self.cancelled.lock().await.remove(operation_id);
    }

''',
    '',
    "remove cancellation-clearing begin operation",
)
write(sftp_path, sftp)

recursive_path = "src-tauri/src/sftp_recursive.rs"
recursive = read(recursive_path)
recursive = recursive.replace('    transfers.begin_operation(&scan_id).await;\n', '')
recursive = replace_once(
    recursive,
    '''    let sftp = open_sftp(&manager, &session_id).await?;
    let result = build_remote_directory_manifest(&sftp, path.trim(), &transfers, &scan_id).await;
    sftp.close().await.ok();
    transfers.finish_operation(&scan_id).await;
    result
''',
    '''    let result = async {
        let sftp = open_sftp(&manager, &session_id).await?;
        let result =
            build_remote_directory_manifest(&sftp, path.trim(), &transfers, &scan_id).await;
        sftp.close().await.ok();
        result
    }
    .await;
    transfers.finish_operation(&scan_id).await;
    result
''',
    "clean remote scan state when opening SFTP fails",
)
recursive = recursive.replace('        transfers.begin_operation("scan-test").await;\n', '')
recursive = recursive.replace('        transfers.begin_operation("scan-cancel").await;\n', '')
recursive = recursive.replace('        transfers.begin_operation("scan-link").await;\n', '')
write(recursive_path, recursive)

# Expose a safe way for the renderer to distinguish a dropped file from a failed directory scan.
service_path = "src/services/ssh.ts"
service = read(service_path)
service = replace_once(
    service,
    '''export function describeCommandError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  return error instanceof Error ? error.message : String(error);
}
''',
    '''export function commandErrorCode(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as CommandError).code);
  }
  return undefined;
}

export function describeCommandError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  return error instanceof Error ? error.message : String(error);
}
''',
    "add command error code helper",
)
write(service_path, service)

app_path = "src/App.vue"
app = read(app_path)
app = replace_once(
    app,
    '''  cancelSftpTransfer,
  connectProfile,
''',
    '''  cancelSftpTransfer,
  commandErrorCode,
  connectProfile,
''',
    "import command error code helper",
)
app = replace_once(
    app,
    '''async function handleDroppedPaths(paths: string[]) {
  const session = activeSession.value;
  if (!session?.connected || !paths.length) return;
  const sessionId = session.id;
  selectedTool.value = "files";
  const files: string[] = [];
  for (const path of paths) {
    try {
      const manifest = await scanLocalDirectory(path, sessionId);
      await uploadDirectoryPath(path, manifest, sessionId);
    } catch {
      files.push(path);
    }
  }
  if (files.length) await uploadFilePaths(files, sessionId);
}
''',
    '''async function handleDroppedPaths(paths: string[]) {
  const session = activeSession.value;
  if (!session?.connected || !paths.length) return;
  const sessionId = session.id;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  selectedTool.value = "files";
  const files: string[] = [];
  for (const path of paths) {
    try {
      const manifest = await scanLocalDirectory(path, sessionId);
      await uploadDirectoryPath(path, manifest, sessionId);
    } catch (error) {
      if (commandErrorCode(error) === "LOCAL_DIRECTORY_INVALID") {
        files.push(path);
        continue;
      }
      state.error = describeCommandError(error);
      return;
    }
  }
  if (files.length) await uploadFilePaths(files, sessionId);
}
''',
    "preserve recursive scan errors during drag and drop",
)
write(app_path, app)

# Keep docs explicit about cancelled/unsafe drag scans not falling back to file upload.
handoff_path = "handoff.md"
handoff = read(handoff_path)
handoff = replace_once(
    handoff,
    "- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总。",
    "- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总；拖放扫描失败不会降级为文件上传。",
    "document drag scan safety",
)
write(handoff_path, handoff)

Path("scripts/apply_pr8_review_fixes.py").unlink()
Path(".github/workflows/apply-pr8-review-fixes.yml").unlink()
