from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


def remove_once(text: str, value: str, label: str) -> str:
    return replace_once(text, value, "", label)


path = Path("src/App.vue")
text = path.read_text(encoding="utf-8")

for name in [
    "deleteSftpTransferCheckpoint",
    "discardSftpTransferCheckpoint",
    "downloadSftpFile",
    "listSftpTransferCheckpoints",
    "listenSftpTransfers",
    "uploadSftpFile",
    "type TransferCheckpoint",
    "type TransferEvent",
]:
    text = remove_once(text, f"  {name},\n", f"remove import {name}")

text = replace_once(
    text,
    "  listenSshEvents,\n",
    "  listenSftpQueueTasks,\n  listenSshEvents,\n",
    "queue event import",
)
text = replace_once(
    text,
    "  sendSshInput,\n",
    "  sendSshInput,\n  wakeSftpTransferQueue,\n",
    "queue wake import",
)
text = replace_once(
    text,
    "  type SystemMetrics,\n",
    "  type SystemMetrics,\n  type TransferQueueTask,\n",
    "queue task type import",
)

session_import_end = '''} from "./sftp/session-state";
import {
  IconAdjustmentsHorizontal,'''
text = replace_once(
    text,
    session_import_end,
    '''} from "./sftp/session-state";
import { useSftpTransferQueue } from "./sftp/transfer-queue";
import {
  IconAdjustmentsHorizontal,''',
    "queue controller import",
)

text = replace_once(
    text,
    '''const transfers = ref<TransferEvent[]>([]);
const pendingTransferCheckpoints = ref<TransferCheckpoint[]>([]);
const recursiveScan = ref<{ id: string; sessionId: string; label: string } | null>(null);''',
    '''const {
  transferConcurrency,
  visibleTransfers,
  handleTransfer: handleQueueTransfer,
  refreshTransferQueue,
  enqueueTransfer,
  waitForTransferTasks,
  pauseTransfer: pauseQueuedTransfer,
  resumeTransfer: resumeQueuedTransfer,
  retryTransfer: retryQueuedTransfer,
  cancelTransfer: cancelQueuedTransfer,
  clearFinishedTransfers: clearCompletedTransfers,
  changeTransferConcurrency,
} = useSftpTransferQueue();
const recursiveScan = ref<{ id: string; sessionId: string; label: string } | null>(null);''',
    "queue state",
)

text = remove_once(
    text,
    '''type TransferTask =
  | { taskId: string; direction: "upload"; sessionId: string; localPath: string; remotePath: string; conflictStrategy: ConflictStrategy; resume: boolean }
  | { taskId: string; direction: "download"; sessionId: string; remotePath: string; localPath: string; conflictStrategy: ConflictStrategy; resume: boolean };
const transferTasks = new Map<string, TransferTask>();

''',
    "frontend transfer task map",
)
text = remove_once(
    text,
    '''const visibleTransfers = computed(() => transfers.value.filter((item) => item.sessionId === activeSessionId.value));
''',
    "legacy visible transfers",
)
text = remove_once(
    text,
    '''const checkpointSession = (checkpoint: TransferCheckpoint) =>
  sessions.value.find((session) => session.connected && session.id === checkpoint.availableSessionId);
''',
    "checkpoint session helper",
)

text = replace_once(
    text,
    '''  if (["connected", "disconnected", "exit", "error"].includes(event.kind)) void refreshTransferCheckpoints();''',
    '''  if (["connected", "disconnected", "exit", "error"].includes(event.kind)) {
    void wakeSftpTransferQueue().catch(() => undefined);
    void refreshTransferQueue().catch(() => undefined);
  }''',
    "SSH queue wake",
)

run_start = text.index("async function runWithConcurrency<T>(")
run_end = text.index("async function runRecursiveScan<T>(", run_start)
text = text[:run_start] + text[run_end:]

transfer_start = text.index("async function startUpload() {")
transfer_end = text.index("async function createRemoteDirectory() {", transfer_start)
new_transfer_block = r'''type FileQueueRequest = {
  direction: "upload" | "download";
  localPath: string;
  remotePath: string;
  conflictStrategy: ConflictStrategy;
  allowPause: boolean;
};

function transferServerLabel(sessionId: string) {
  return sessions.value.find((session) => session.id === sessionId)?.name ?? "未命名服务器";
}

async function enqueueAndWaitFileTransfers(sessionId: string, requests: FileQueueRequest[]) {
  const taskIds: string[] = [];
  let enqueueError: unknown;
  for (const request of requests) {
    try {
      const task = await enqueueTransfer({
        sessionId,
        serverLabel: transferServerLabel(sessionId),
        ...request,
      });
      taskIds.push(task.taskId);
    } catch (error) {
      enqueueError = error;
      break;
    }
  }

  let transferError: unknown;
  if (taskIds.length) {
    try {
      await waitForTransferTasks(taskIds);
    } catch (error) {
      transferError = error;
    }
  }
  if (enqueueError) throw enqueueError;
  if (transferError) throw transferError;
}

async function startUpload() {
  const session = activeSession.value;
  if (!session?.connected) return;
  const sessionId = session.id;
  const selected = await open({ multiple: true, directory: false, title: "选择要上传的文件" });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await uploadFilePaths(paths, sessionId);
}

async function uploadFilePaths(
  paths: string[],
  sessionId = activeSessionId.value,
  conflicts = createConflictBatchContext(),
) {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const targetDirectory = state.path;
  const requests: FileQueueRequest[] = [];
  for (const localPath of paths) {
    const fileName = localPath.split(/[\\/]/).pop();
    if (!fileName) continue;
    const remotePath = joinRemotePath(targetDirectory, fileName);
    const inspection = await inspectRemotePath(sessionId, remotePath);
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
    requests.push({
      direction: "upload",
      localPath,
      remotePath,
      conflictStrategy,
      allowPause: true,
    });
  }
  try {
    await enqueueAndWaitFileTransfers(sessionId, requests);
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function startUploadDirectory() {
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
    prepared = await prepareRemoteDirectory(
      sessionId,
      requestedRoot,
      directoryStrategy,
      directoryStrategy === "replace" ? crypto.randomUUID() : undefined,
    );
    if (prepared.skipped) return;

    for (const directory of manifest.directories) {
      await prepareRemoteDirectory(sessionId, joinRemotePath(prepared.path, directory), "merge");
    }
    const requests: FileQueueRequest[] = [];
    for (const file of manifest.files) {
      const remotePath = joinRemotePath(prepared.path, file.relativePath);
      const inspection = await inspectRemotePath(sessionId, remotePath);
      if (inspection.kind === "directory" || inspection.kind === "symlink" || inspection.kind === "other") {
        throw new Error(`远程目标“${file.relativePath}”已存在同名目录、链接或不支持的条目`);
      }
      let conflictStrategy: ConflictStrategy = "overwrite";
      if (inspection.kind === "file") {
        const choice = await chooseFileConflict(file.relativePath, conflicts, true);
        if (choice === "cancel") {
          await finishPreparedDirectory(prepared, false, sessionId);
          prepared = undefined;
          return;
        }
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      requests.push({
        direction: "upload",
        localPath: file.absolutePath,
        remotePath,
        conflictStrategy,
        allowPause: false,
      });
    }
    await enqueueAndWaitFileTransfers(sessionId, requests);
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
  if (inspection.kind === "file" || inspection.kind === "symlink" || inspection.kind === "other") {
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
    for (const directory of manifest.directories) {
      await prepareLocalDirectory(joinLocalPath(prepared.path, directory), "merge");
    }
    const requests: FileQueueRequest[] = [];
    for (const file of manifest.files) {
      const localPath = joinLocalPath(prepared.path, file.relativePath);
      const inspection = await inspectLocalPath(localPath);
      if (inspection.kind === "directory" || inspection.kind === "symlink" || inspection.kind === "other") {
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
      requests.push({
        direction: "download",
        remotePath: file.remotePath,
        localPath,
        conflictStrategy,
        allowPause: false,
      });
    }
    await enqueueAndWaitFileTransfers(sessionId, requests);
    await finishPreparedDirectory(prepared, true);
    return true;
  } catch (error) {
    await rollbackPreparedDirectory(prepared, undefined, error);
  }
}

async function startDownload() {
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
    const directRequests: FileQueueRequest[] = [];
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
      if (inspection.kind === "directory" || inspection.kind === "symlink" || inspection.kind === "other") {
        throw new Error(`本地目标“${item.name}”已存在同名目录、链接或不支持的条目`);
      }
      let conflictStrategy: ConflictStrategy = "overwrite";
      if (inspection.kind === "file") {
        const choice = await chooseFileConflict(item.name, conflicts, selected.length > 1);
        if (choice === "cancel") return;
        conflictStrategy = choice;
        if (choice === "skip") continue;
      }
      directRequests.push({
        direction: "download",
        remotePath: item.path,
        localPath,
        conflictStrategy,
        allowPause: true,
      });
    }
    await enqueueAndWaitFileTransfers(sessionId, directRequests);
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function downloadOne(
  sessionId: string,
  remotePath: string,
  localPath: string,
  conflictStrategy: ConflictStrategy,
) {
  await enqueueAndWaitFileTransfers(sessionId, [{
    direction: "download",
    remotePath,
    localPath,
    conflictStrategy,
    allowPause: true,
  }]);
}

function handleTransfer(task: TransferQueueTask) {
  handleQueueTransfer(task);
  if (task.state === "failed" && task.sessionId) {
    const state = sftpStates.get(task.sessionId);
    if (state) state.error = task.message ?? "文件传输失败";
  }
}

function transferProgress(item: TransferQueueTask) {
  return item.total > 0 ? Math.round((item.transferred / item.total) * 100) : 0;
}

function transferStatusText(item: TransferQueueTask) {
  if (item.state === "queued") return item.availableSessionId ? "排队中" : "等待连接";
  if (item.state === "running") return `${transferProgress(item)}%`;
  if (item.state === "pausing") return "正在停止";
  if (item.state === "paused") return "已暂停";
  if (item.state === "completed") return "完成";
  if (item.state === "failed") return "失败";
  return "已取消";
}

function reportTransferActionError(item: TransferQueueTask, error: unknown) {
  const message = describeCommandError(error);
  const sessionId = item.availableSessionId ?? item.sessionId;
  const state = sessionId ? sftpStates.get(sessionId) : undefined;
  if (state) state.error = message;
  else window.alert(message);
}

async function pauseTransferTask(item: TransferQueueTask) {
  try {
    await pauseQueuedTransfer(item);
  } catch (error) {
    reportTransferActionError(item, error);
  }
}

async function resumeTransferTask(item: TransferQueueTask) {
  if (item.availableSessionId) activeSessionId.value = item.availableSessionId;
  try {
    await resumeQueuedTransfer(item);
  } catch (error) {
    reportTransferActionError(item, error);
  }
}

async function retryTransferTask(item: TransferQueueTask) {
  if (item.availableSessionId) activeSessionId.value = item.availableSessionId;
  try {
    await retryQueuedTransfer(item);
  } catch (error) {
    reportTransferActionError(item, error);
  }
}

async function cancelTransferTask(item: TransferQueueTask, deletePartial: boolean) {
  if (deletePartial) {
    const confirmed = await ask(
      `确定取消“${item.fileName}”并删除已经保存的断点和临时文件吗？`,
      {
        title: "删除传输断点",
        kind: "warning",
        okLabel: "取消并删除",
        cancelLabel: "返回",
      },
    );
    if (!confirmed) return;
  }
  try {
    await cancelQueuedTransfer(item, deletePartial);
  } catch (error) {
    reportTransferActionError(item, error);
  }
}

async function clearFinishedTransfers() {
  try {
    await clearCompletedTransfers();
  } catch (error) {
    window.alert(describeCommandError(error));
  }
}

async function updateTransferConcurrency(event: Event) {
  const concurrency = Number((event.target as HTMLSelectElement).value);
  try {
    await changeTransferConcurrency(concurrency);
  } catch (error) {
    window.alert(describeCommandError(error));
  }
}

'''
text = text[:transfer_start] + new_transfer_block + text[transfer_end:]

old_queue_start = text.index('          <div v-if="pendingTransferCheckpoints.length" class="transfer-queue checkpoint-queue">')
old_queue_end = text.index('          <div v-if="activeSftpState.notice"', old_queue_start)
new_queue_template = '''          <div v-if="visibleTransfers.length" class="transfer-queue">
            <div class="transfer-queue-heading">
              <span>传输队列（{{ visibleTransfers.length }}）</span>
              <label class="transfer-concurrency">并发
                <select :value="transferConcurrency" aria-label="传输并发数" @change="updateTransferConcurrency">
                  <option v-for="value in 5" :key="value" :value="value">{{ value }}</option>
                </select>
              </label>
              <button @click="clearFinishedTransfers">清除已完成</button>
            </div>
            <div v-for="item in visibleTransfers" :key="item.taskId" class="upload-strip" :class="item.state">
              <span>{{ item.direction === 'upload' ? '上传' : '下载' }} {{ item.fileName }}
                <small>{{ item.serverLabel }} · {{ item.sourcePath }} → {{ item.targetPath }}</small>
                <small v-if="item.resumedFrom">已续传 {{ formatSize(item.resumedFrom, 'file') }}</small>
              </span>
              <div><i :style="{ width: `${transferProgress(item)}%` }"></i></div>
              <span class="transfer-rate">{{ item.state === 'running' ? `${formatRate(item.speedBytesPerSecond)} · 剩余 ${formatEta(item.etaSeconds)}` : (item.message ?? '') }}</span>
              <strong>{{ transferStatusText(item) }}</strong>
              <button v-if="item.state === 'running' && item.allowPause" @click="pauseTransferTask(item)">暂停</button>
              <button v-if="item.state === 'paused'" @click="resumeTransferTask(item)">继续</button>
              <button v-if="item.state === 'failed' || item.state === 'cancelled'" @click="retryTransferTask(item)">重试</button>
              <button v-if="item.state === 'queued' || item.state === 'running' || item.state === 'paused'" @click="cancelTransferTask(item, false)">取消并保留</button>
              <button v-if="item.state !== 'completed' && item.state !== 'pausing' && item.checkpointAvailable" class="danger-button" @click="cancelTransferTask(item, true)">删除断点</button>
            </div>
          </div>
'''
text = text[:old_queue_start] + new_queue_template + text[old_queue_end:]

text = replace_once(
    text,
    '''    unlistenSftp = await listenSftpTransfers(handleTransfer);''',
    '''    unlistenSftp = await listenSftpQueueTasks(handleTransfer);''',
    "queue listener mount",
)
text = replace_once(
    text,
    '''    await refreshTransferCheckpoints();''',
    '''    await refreshTransferQueue();''',
    "queue initial refresh",
)

path.write_text(text, encoding="utf-8")
