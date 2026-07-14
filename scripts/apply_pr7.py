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


def patch_directory_module() -> None:
    path = "src-tauri/src/sftp_directory.rs"
    text = read(path)
    text = replace_once(
        text,
        '''        if transactions
            .insert(replacement_id.to_owned(), replacement)
            .is_some()
        {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        Ok(())
''',
        '''        if transactions.contains_key(replacement_id) {
            return Err(CommandError::new(
                "DIRECTORY_REPLACEMENT_BUSY",
                "该目录替换任务已经存在",
            ));
        }
        transactions.insert(replacement_id.to_owned(), replacement);
        Ok(())
''',
        "transaction register",
    )
    write(path, text)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    text = read(path)
    text = replace_once(text, "mod sftp;\nmod sftp_recursive;", "mod sftp;\nmod sftp_directory;\nmod sftp_recursive;", "directory module")
    text = replace_once(
        text,
        '''use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive,
    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_prepare_local_directory, sftp_rename, sftp_upload,
    SftpTransferManager,
};
use sftp_recursive::{sftp_local_directory_manifest, sftp_remote_directory_manifest};
''',
        '''use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive,
    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_rename, sftp_upload, SftpTransferManager,
};
use sftp_directory::{
    sftp_finish_directory_replacement, sftp_inspect_local_path, sftp_prepare_local_directory,
    sftp_prepare_remote_directory, DirectoryReplacementManager,
};
use sftp_recursive::{sftp_local_directory_manifest, sftp_remote_directory_manifest};
''',
        "directory imports",
    )
    text = replace_once(
        text,
        "        .manage(SftpTransferManager::default())\n",
        "        .manage(SftpTransferManager::default())\n        .manage(DirectoryReplacementManager::default())\n",
        "directory state",
    )
    text = replace_once(
        text,
        '''            sftp_remote_directory_manifest,
            sftp_prepare_local_directory,
            sftp_create_directory,
''',
        '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_prepare_local_directory,
            sftp_prepare_remote_directory,
            sftp_finish_directory_replacement,
            sftp_create_directory,
''',
        "directory commands",
    )
    write(path, text)


def patch_sftp() -> None:
    path = "src-tauri/src/sftp.rs"
    text = read(path)
    text = replace_section(
        text,
        "#[tauri::command]\npub async fn sftp_prepare_local_directory(",
        "#[tauri::command]\npub async fn sftp_list(",
        "",
        "remove legacy local directory prepare",
    )
    text = replace_section(
        text,
        "    #[tokio::test]\n    async fn refuses_to_prepare_a_directory_over_an_existing_file()",
        "    #[test]\n    fn maps_transfer_errors_to_one_terminal_state()",
        "",
        "remove legacy directory test",
    )
    write(path, text)


def patch_service() -> None:
    path = "src/services/ssh.ts"
    text = read(path)
    text = replace_once(
        text,
        '''export type ConflictStrategy = "overwrite" | "skip" | "rename";

export type TransferResult = {
''',
        '''export type ConflictStrategy = "overwrite" | "skip" | "rename";
export type DirectoryConflictStrategy = "merge" | "skip" | "rename" | "replace";
export type LocalPathKind = "missing" | "file" | "directory" | "other";

export type DirectoryPrepareResult = {
  path: string;
  skipped: boolean;
  existed: boolean;
  replacementId?: string;
};

export type TransferResult = {
''',
        "directory types",
    )
    text = replace_once(
        text,
        '''export const prepareLocalDirectory = (path: string, conflictStrategy: ConflictStrategy = "overwrite") =>
  invoke<TransferResult>("sftp_prepare_local_directory", { path, conflictStrategy });

export const createSftpDirectory = (sessionId: string, path: string) =>
''',
        '''export const inspectLocalPath = (path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_local_path", { path });

export const prepareLocalDirectory = (
  path: string,
  conflictStrategy: DirectoryConflictStrategy = "merge",
  replacementId?: string,
) => invoke<DirectoryPrepareResult>("sftp_prepare_local_directory", { path, conflictStrategy, replacementId });

export const prepareRemoteDirectory = (
  sessionId: string,
  path: string,
  conflictStrategy: DirectoryConflictStrategy = "merge",
  replacementId?: string,
) => invoke<DirectoryPrepareResult>("sftp_prepare_remote_directory", {
  sessionId,
  path,
  conflictStrategy,
  replacementId,
});

export const finishDirectoryReplacement = (
  replacementId: string,
  commit: boolean,
  sessionId?: string,
) => invoke<void>("sftp_finish_directory_replacement", { replacementId, commit, sessionId });

export const createSftpDirectory = (sessionId: string, path: string) =>
''',
        "directory service commands",
    )
    write(path, text)


def patch_app() -> None:
    path = "src/App.vue"
    text = read(path)
    text = replace_once(text, "  fetchSystemMetrics,\n", "  fetchSystemMetrics,\n  finishDirectoryReplacement,\n", "finish import")
    text = replace_once(text, "  isTauri,\n", "  inspectLocalPath,\n  isTauri,\n", "inspect import")
    text = replace_once(text, "  prepareLocalDirectory,\n", "  prepareLocalDirectory,\n  prepareRemoteDirectory,\n", "remote prepare import")
    text = replace_once(text, "  type ConflictStrategy,\n", "  type ConflictStrategy,\n  type DirectoryConflictStrategy,\n  type DirectoryPrepareResult,\n", "directory type imports")
    text = replace_once(text, "  type LocalDirectoryManifest,\n", "  type LocalDirectoryManifest,\n  type LocalPathKind,\n", "local path type import")
    text = replace_once(
        text,
        '''const conflictRequest = ref<{ name: string; allowAll: boolean; resolve: (result: { strategy: ConflictStrategy | "cancel"; applyAll: boolean }) => void } | null>(null);
const conflictApplyAll = ref(false);
''',
        '''type ConflictKind = "file" | "directory";
type ConflictValue = ConflictStrategy | DirectoryConflictStrategy;
type ConflictResolution = { strategy: ConflictValue | "cancel"; applyAll: boolean };
type ConflictBatchContext = {
  fileStrategy: ConflictStrategy | null;
  directoryStrategy: DirectoryConflictStrategy | null;
};
const conflictRequest = ref<{
  kind: ConflictKind;
  name: string;
  allowAll: boolean;
  resolve: (result: ConflictResolution) => void;
} | null>(null);
const conflictApplyAll = ref(false);
''',
        "conflict state",
    )
    text = replace_section(
        text,
        "function requestConflict(name: string, allowAll = false)",
        "function sftpStorageScope(sessionId: string)",
        '''function createConflictBatchContext(): ConflictBatchContext {
  return { fileStrategy: null, directoryStrategy: null };
}

function requestConflict(kind: ConflictKind, name: string, allowAll = false) {
  conflictApplyAll.value = false;
  return new Promise<ConflictResolution>((resolve) => {
    conflictRequest.value = { kind, name, allowAll, resolve };
  });
}

async function resolveConflict(strategy: ConflictValue | "cancel") {
  const request = conflictRequest.value;
  if (!request) return;
  if (strategy === "replace") {
    const confirmed = await ask(
      `替换目录“${request.name}”会删除目标中源目录不存在的额外内容。原目录会先安全备份，复制失败时自动恢复。确定继续吗？`,
      {
        title: "确认替换目录",
        kind: "warning",
        okLabel: "替换目录",
        cancelLabel: "返回",
      },
    );
    if (!confirmed) return;
  }
  request.resolve({ strategy, applyAll: request.allowAll && conflictApplyAll.value });
  conflictRequest.value = null;
}

async function chooseFileConflict(
  name: string,
  context: ConflictBatchContext,
  allowAll: boolean,
): Promise<ConflictStrategy | "cancel"> {
  if (context.fileStrategy) return context.fileStrategy;
  const result = await requestConflict("file", name, allowAll);
  if (result.strategy === "cancel") return "cancel";
  if (!(["overwrite", "skip", "rename"] as ConflictValue[]).includes(result.strategy)) {
    throw new Error("文件冲突策略无效");
  }
  const strategy = result.strategy as ConflictStrategy;
  if (result.applyAll) context.fileStrategy = strategy;
  return strategy;
}

async function chooseDirectoryConflict(
  name: string,
  context: ConflictBatchContext,
  allowAll: boolean,
): Promise<DirectoryConflictStrategy | "cancel"> {
  if (context.directoryStrategy) return context.directoryStrategy;
  const result = await requestConflict("directory", name, allowAll);
  if (result.strategy === "cancel") return "cancel";
  if (!(["merge", "skip", "rename", "replace"] as ConflictValue[]).includes(result.strategy)) {
    throw new Error("目录冲突策略无效");
  }
  const strategy = result.strategy as DirectoryConflictStrategy;
  if (result.applyAll) context.directoryStrategy = strategy;
  return strategy;
}

async function finishPreparedDirectory(
  prepared: DirectoryPrepareResult | undefined,
  commit: boolean,
  sessionId?: string,
) {
  if (!prepared?.replacementId) return;
  await finishDirectoryReplacement(prepared.replacementId, commit, sessionId);
}

async function rollbackPreparedDirectory(
  prepared: DirectoryPrepareResult | undefined,
  sessionId: string | undefined,
  cause: unknown,
): Promise<never> {
  if (prepared?.replacementId) {
    try {
      await finishPreparedDirectory(prepared, false, sessionId);
    } catch (rollbackError) {
      throw new Error(`${describeCommandError(cause)}；自动恢复原目录失败：${describeCommandError(rollbackError)}`);
    }
  }
  throw cause;
}

function sftpStorageScope(sessionId: string)''',
        "conflict functions",
    )
    text = replace_section(
        text,
        "async function uploadFilePaths(",
        "async function startUploadDirectory()",
        '''async function uploadFilePaths(
  paths: string[],
  sessionId = activeSessionId.value,
  conflicts = createConflictBatchContext(),
) {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const targetDirectory = state.path;
  const tasks: Array<{ localPath: string; remotePath: string; conflictStrategy: ConflictStrategy }> = [];
  for (const localPath of paths) {
    const fileName = localPath.split(/[\\/]/).pop();
    if (!fileName) continue;
    const remotePath = joinRemotePath(targetDirectory, fileName);
    const existing = state.entries.find((entry) => entry.name === fileName);
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
    tasks.push({ localPath, remotePath, conflictStrategy });
  }
  try {
    await runWithConcurrency(tasks, async (task) => {
      const transferId = crypto.randomUUID();
      const taskId = crypto.randomUUID();
      const request = { taskId, direction: "upload" as const, sessionId, ...task, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, transferId, taskId, ...task, resume: false });
      if (result.skipped) transferTasks.delete(transferId);
    });
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function startUploadDirectory()''',
        "upload files",
    )
    text = replace_section(
        text,
        "async function startUploadDirectory()",
        "async function startDownload()",
        '''async function startUploadDirectory() {
  const session = activeSession.value;
  if (!session?.connected) return;
  const sessionId = session.id;
  const localPath = await open({ directory: true, multiple: false, title: "选择要上传的文件夹" });
  if (!localPath || Array.isArray(localPath)) return;
  await uploadDirectoryPath(
    localPath,
    undefined,
    sessionId,
    createConflictBatchContext(),
    false,
  );
}

async function uploadDirectoryPath(
  localPath: string,
  knownManifest?: LocalDirectoryManifest,
  sessionId = activeSessionId.value,
  conflicts = createConflictBatchContext(),
  allowDirectoryAll = false,
) {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const targetDirectory = state.path;
  let prepared: DirectoryPrepareResult | undefined;
  try {
    const manifest = knownManifest ?? await scanLocalDirectory(localPath, sessionId);
    updateRecursiveScanNotice(state, manifest);
    const requestedRoot = joinRemotePath(targetDirectory, manifest.rootName);
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
    prepared = await prepareRemoteDirectory(
      sessionId,
      requestedRoot,
      directoryStrategy,
      directoryStrategy === "replace" ? crypto.randomUUID() : undefined,
    );
    if (prepared.skipped) return;

    const existingFiles = new Set<string>();
    if (prepared.existed && directoryStrategy === "merge") {
      const existingManifest = await scanRemoteDirectory(sessionId, prepared.path);
      for (const file of existingManifest.files) existingFiles.add(file.relativePath);
    }
    for (const directory of manifest.directories) {
      await createSftpDirectory(sessionId, joinRemotePath(prepared.path, directory));
    }
    const tasks: Array<{
      localPath: string;
      remotePath: string;
      conflictStrategy: ConflictStrategy;
    }> = [];
    for (const file of manifest.files) {
      let conflictStrategy: ConflictStrategy = "overwrite";
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
    }
    try {
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
}

async function downloadDirectoryItem(
  sessionId: string,
  state: SftpSessionState,
  item: SessionSftpEntry,
  localRoot: string,
  conflicts: ConflictBatchContext,
  allowDirectoryAll: boolean,
) {
  const targetPath = joinLocalPath(localRoot, item.name);
  const inspection = await inspectLocalPath(targetPath);
  if (inspection.kind === "file" || inspection.kind === "other") {
    throw new Error(`本地目标“${item.name}”已存在同名文件、链接或不支持的条目`);
  }
  let directoryStrategy: DirectoryConflictStrategy = "merge";
  if (inspection.kind === "directory") {
    const choice = await chooseDirectoryConflict(item.name, conflicts, allowDirectoryAll);
    if (choice === "cancel") return false;
    if (choice === "skip") return true;
    directoryStrategy = choice;
  }
  const prepared = await prepareLocalDirectory(
    targetPath,
    directoryStrategy,
    directoryStrategy === "replace" ? crypto.randomUUID() : undefined,
  );
  if (prepared.skipped) return true;
  try {
    const manifest = await scanRemoteDirectory(sessionId, item.path);
    updateRecursiveScanNotice(state, manifest);
    const existingFiles = new Set<string>();
    if (prepared.existed && directoryStrategy === "merge") {
      const localManifest = await scanLocalDirectory(prepared.path, sessionId);
      for (const file of localManifest.files) existingFiles.add(file.relativePath);
    }
    for (const directory of manifest.directories) {
      await prepareLocalDirectory(joinLocalPath(prepared.path, directory), "merge");
    }
    const downloads: Array<{
      remotePath: string;
      localPath: string;
      conflictStrategy: ConflictStrategy;
    }> = [];
    for (const file of manifest.files) {
      let conflictStrategy: ConflictStrategy = "overwrite";
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
    }
    try {
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
    return true;
  } catch (error) {
    await rollbackPreparedDirectory(prepared, undefined, error);
  }
}

async function startDownload()''',
        "directory upload and helper",
    )
    text = replace_section(
        text,
        "async function startDownload()",
        "async function downloadOne(",
        '''async function startDownload() {
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
  const conflicts = createConflictBatchContext();
  try {
    const directDownloads: Array<{
      remotePath: string;
      localPath: string;
      conflictStrategy: ConflictStrategy;
    }> = [];
    const directoryCount = selected.filter((item) => item.kind === "directory").length;
    for (const item of selected) {
      if (item.kind === "directory") {
        const completed = await downloadDirectoryItem(
          sessionId,
          state,
          item,
          localRoot,
          conflicts,
          directoryCount > 1,
        );
        if (!completed) return;
        continue;
      }
      if (item.kind !== "file") continue;
      const localPath = joinLocalPath(localRoot, item.name);
      const inspection = await inspectLocalPath(localPath);
      if (inspection.kind === "directory" || inspection.kind === "other") {
        throw new Error(`本地目标“${item.name}”已存在同名目录、链接或不支持的条目`);
      }
      let conflictStrategy: ConflictStrategy = "overwrite";
      if (inspection.kind === "file") {
        const choice = await chooseFileConflict(item.name, conflicts, selected.length > 1);
        if (choice === "cancel") return;
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      directDownloads.push({ remotePath: item.path, localPath, conflictStrategy });
    }
    await runWithConcurrency(directDownloads, (download) => downloadOne(
      sessionId,
      download.remotePath,
      download.localPath,
      download.conflictStrategy,
    ));
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function downloadOne(''',
        "download flow",
    )
    text = replace_section(
        text,
        "async function handleDroppedPaths(paths: string[])",
        "function formatUptime(seconds = 0)",
        '''async function handleDroppedPaths(paths: string[]) {
  const session = activeSession.value;
  if (!session?.connected || !paths.length) return;
  const sessionId = session.id;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const conflicts = createConflictBatchContext();
  selectedTool.value = "files";
  const files: string[] = [];
  for (const path of paths) {
    try {
      const manifest = await scanLocalDirectory(path, sessionId);
      await uploadDirectoryPath(path, manifest, sessionId, conflicts, paths.length > 1);
    } catch (error) {
      if (commandErrorCode(error) === "LOCAL_DIRECTORY_INVALID") {
        files.push(path);
        continue;
      }
      state.error = describeCommandError(error);
      return;
    }
  }
  if (files.length) await uploadFilePaths(files, sessionId, conflicts);
}

function formatUptime(seconds = 0)''',
        "drop flow",
    )
    text = replace_once(
        text,
        '''    <div v-if="conflictRequest" class="dialog-backdrop conflict-backdrop">
      <section class="conflict-dialog" role="dialog" aria-modal="true" aria-label="同名文件处理"><header><strong>目标中已存在同名项目</strong></header><p>“{{ conflictRequest.name }}”已存在，请选择处理方式。</p><label v-if="conflictRequest.allowAll"><input v-model="conflictApplyAll" type="checkbox" />应用到本批次全部冲突</label><footer><button @click="resolveConflict('cancel')">取消</button><button @click="resolveConflict('skip')">跳过</button><button @click="resolveConflict('rename')">自动重命名</button><button class="primary-button" @click="resolveConflict('overwrite')">安全覆盖</button></footer></section>
    </div>
''',
        '''    <div v-if="conflictRequest" class="dialog-backdrop conflict-backdrop">
      <section class="conflict-dialog" role="dialog" aria-modal="true" :aria-label="conflictRequest.kind === 'directory' ? '同名目录处理' : '同名文件处理'">
        <header><strong>{{ conflictRequest.kind === 'directory' ? '目标中已存在同名目录' : '目标中已存在同名文件' }}</strong></header>
        <p>“{{ conflictRequest.name }}”已存在，请选择处理方式。</p>
        <p v-if="conflictRequest.kind === 'directory'" class="conflict-explanation">合并会保留目标中的额外内容；替换会先备份原目录，并删除目标中源目录不存在的额外内容。</p>
        <label v-if="conflictRequest.allowAll"><input v-model="conflictApplyAll" type="checkbox" />应用到本批次全部{{ conflictRequest.kind === 'directory' ? '目录' : '文件' }}冲突</label>
        <footer>
          <button @click="resolveConflict('cancel')">取消</button>
          <button @click="resolveConflict('skip')">跳过</button>
          <button @click="resolveConflict('rename')">自动重命名</button>
          <button v-if="conflictRequest.kind === 'directory'" class="danger-button" @click="resolveConflict('replace')">替换目录</button>
          <button v-if="conflictRequest.kind === 'directory'" class="primary-button" @click="resolveConflict('merge')">合并目录</button>
          <button v-else class="primary-button" @click="resolveConflict('overwrite')">安全覆盖</button>
        </footer>
      </section>
    </div>
''',
        "conflict dialog",
    )
    write(path, text)


def patch_styles() -> None:
    path = "src/styles.css"
    text = read(path)
    addition = '''
.conflict-explanation { color: #a9bac3; font-size: 12px; line-height: 1.55; }
.conflict-dialog .danger-button { color: #ffd9dc; border-color: #8a4148; background: #5a2930; }
.conflict-dialog .danger-button:hover { background: #71343c; }
'''
    if ".conflict-explanation" not in text:
        text = text.rstrip() + "\n" + addition
    write(path, text)


def patch_docs() -> None:
    plan = read("plan.md")
    plan = replace_once(
        plan,
        "状态：实施进行中，PR1～PR5 已完成，PR6 已实现并待验证，下一步为 PR7",
        "状态：实施进行中，PR1～PR6 已完成，PR7 已实现并待验证",
        "plan summary status",
    )
    plan = replace_once(
        plan,
        "| PR6 | 递归传输和符号链接安全 | `fix/sftp-recursive-transfer-safety` | 待验证 | PR5 |\n| PR7 | 明确目录冲突语义 | `feat/sftp-directory-conflict-strategies` | 待开始 | PR6 |",
        "| PR6 | 递归传输和符号链接安全 | `fix/sftp-recursive-transfer-safety` | 已完成 | PR5 |\n| PR7 | 明确目录冲突语义 | `feat/sftp-directory-conflict-strategies` | 待验证 | PR6 |",
        "plan table status",
    )
    plan = replace_once(
        plan,
        "验证：等待 GitHub Actions；未执行真实服务器写入测试。\n\n本地待测：Windows junction 循环、远程 symlink 指向根外、深层目录、超大量小文件和扫描取消。\n\n下一步：PR7：明确目录冲突语义。",
        "验证：PR #8 Frontend 与 Rust CI 已通过并 squash 合并；未执行真实服务器写入测试。\n\n本地待测：Windows junction 循环、远程 symlink 指向根外、深层目录、超大量小文件和扫描取消。\n\n下一步：PR7：明确目录冲突语义。",
        "PR6 validation",
    )
    plan = replace_once(plan, "## 11. PR7：明确目录冲突语义\n\n状态：`待开始`", "## 11. PR7：明确目录冲突语义\n\n状态：`待验证`", "PR7 status")
    plan = replace_once(
        plan,
        "### 完成记录\n\n尚未开始。\n\n---\n\n## 12. PR8：后端统一传输队列、暂停和恢复",
        '''### 完成记录

实现内容：

- 文件冲突继续使用覆盖、跳过和重命名，目录冲突独立为合并、跳过、重命名和替换。
- 文件与目录的批次“应用到全部”状态分开维护，不再共用同一个策略。
- 本地和远程目录统一通过后端准备命令处理，重命名生成稳定的新目录名称。
- 目录替换使用不透明 replacement ID 绑定目标、备份和服务器身份。
- 替换前先将原目录安全 rename 到备份；复制失败自动删除新目录并恢复原目录。
- 复制成功后提交删除备份；服务器不支持安全 rename 时明确拒绝，不会静默递归删除原目录。
- UI 明确说明合并保留额外内容、替换删除额外内容，并对替换进行二次确认。
- 新增本地合并、跳过、重命名、替换提交和失败回滚测试。

验证：等待 GitHub Actions；未执行真实服务器写入测试。

本地待测：本地目录替换、远程目录 merge/replace、远程 rename 不支持、替换中断与恢复。

下一步：PR8：后端统一传输队列、暂停和恢复。

---

## 12. PR8：后端统一传输队列、暂停和恢复''',
        "PR7 completion",
    )
    write("plan.md", plan)

    readme = read("README.md")
    readme = replace_once(
        readme,
        "- 递归上传和下载默认跳过本地 symlink、Windows junction/reparse point 与远程 symlink，并限制深度、数量和累计大小。",
        "- 递归上传和下载默认跳过本地 symlink、Windows junction/reparse point 与远程 symlink，并限制深度、数量和累计大小。\n- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和事务式替换。",
        "README conflict feature",
    )
    readme = replace_once(
        readme,
        "- PR1～PR6 已完成；下一步按 `plan.md` 处理 PR7 目录冲突语义。",
        "- PR1～PR7 已实现；PR7 正在等待 CI、code review 和合并，下一步按 `plan.md` 处理 PR8 后端统一传输队列。",
        "README progress",
    )
    write("README.md", readme)

    handoff = read("handoff.md")
    handoff = replace_once(
        handoff,
        "当前状态：PR1～PR5 已合并；PR6 递归传输和符号链接安全已实现，CI 与 code review 已通过，等待合并。PR6 合并后进入 PR7。",
        "当前状态：PR1～PR6 已合并；PR7 目录冲突语义已实现，正在等待 CI、code review 和合并。PR7 合并后进入 PR8。",
        "handoff status",
    )
    handoff = replace_once(
        handoff,
        "- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总；拖放扫描失败不会降级为文件上传。",
        "- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总；拖放扫描失败不会降级为文件上传。\n- PR7：文件与目录冲突策略分离，目录支持合并/跳过/重命名/事务式替换，替换失败自动恢复原目录。",
        "handoff feature",
    )
    handoff = replace_section(
        handoff,
        "## 6. 当前任务与下一任务",
        "## 7. 关键文件",
        '''## 6. 当前任务与下一任务

当前任务：PR7 明确目录冲突语义，分支 `feat/sftp-directory-conflict-strategies`，状态为待验证。

PR7 已处理：

- 文件冲突继续使用覆盖、跳过和重命名。
- 目录冲突独立使用合并、跳过、重命名和替换。
- 文件与目录的批次“应用到全部”策略分开保存。
- 本地和远程目录替换先安全备份，复制失败时回滚恢复，成功后提交清理备份。
- 远程服务器不支持安全 rename 时明确拒绝替换，不递归删除原目录。
- UI 说明合并与替换的实际含义，并对替换执行二次确认。

PR7 不处理：

- 后端统一传输队列、暂停与恢复。
- SFTP 导航、批量操作和拖放作用域完善。

PR7 合并后下一任务为 PR8：后端统一传输队列、暂停和恢复，分支建议 `feat/sftp-transfer-queue`.''',
        "handoff current task",
    )
    handoff = replace_once(
        handoff,
        "- `src-tauri/src/sftp_recursive.rs`\n  - PR6 本地/远程受限递归扫描、链接跳过、边界、限制、取消和汇总。",
        "- `src-tauri/src/sftp_recursive.rs`\n  - PR6 本地/远程受限递归扫描、链接跳过、边界、限制、取消和汇总。\n- `src-tauri/src/sftp_directory.rs`\n  - PR7 文件/目录冲突分离和本地/远程事务式目录替换。",
        "handoff key file",
    )
    write("handoff.md", handoff)


def main() -> None:
    patch_directory_module()
    patch_lib()
    patch_sftp()
    patch_service()
    patch_app()
    patch_styles()
    patch_docs()
    Path("scripts/apply_pr7.py").unlink()
    Path(".github/workflows/apply-pr7.yml").unlink()


if __name__ == "__main__":
    main()
