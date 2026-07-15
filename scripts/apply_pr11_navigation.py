from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")

app = replace_once(
    app,
    '''import { useSftpTransferQueue } from "./sftp/transfer-queue";''',
    '''import { useSftpTransferQueue } from "./sftp/transfer-queue";
import {
  buildRemoteBreadcrumbs,
  isPointInsideRect,
  reconcileSelection,
  updateSelectionPaths,
} from "./sftp/navigation-state";''',
    "navigation imports",
)

app = replace_once(
    app,
    '''const sftpSearch = ref("");
const sftpSortKey = ref<"name" | "size" | "modifiedAt">("name");
const sftpSortAscending = ref(true);
const sftpDragActive = ref(false);''',
    '''const sftpSearch = ref("");
const sftpSortKey = ref<"name" | "size" | "modifiedAt">("name");
const sftpSortAscending = ref(true);
const sftpDragActive = ref(false);
const sftpPaneElement = ref<HTMLElement | null>(null);
const sftpPathInput = ref<HTMLInputElement | null>(null);
const sftpPathEditing = ref(false);
const sftpPathDraft = ref("");
const sftpContextMenu = ref<{ x: number; y: number; sessionId: string; entryPath: string } | null>(null);
const fileDoubleClickAction = ref<"select" | "download">(
  localStorage.getItem("liteshell.sftp.file-double-click.v1") === "download" ? "download" : "select",
);
type DroppedDirectoryPreview = { path: string; manifest: LocalDirectoryManifest };
type SftpDropPreview = {
  sessionId: string;
  serverLabel: string;
  targetPath: string;
  files: string[];
  directories: DroppedDirectoryPreview[];
  fileCount: number;
  directoryCount: number;
  totalSize: number;
  skippedCount: number;
};
const sftpDropPreview = ref<SftpDropPreview | null>(null);
const sftpDropBusy = ref(false);''',
    "navigation refs",
)

app = replace_once(
    app,
    '''const selectedRemoteFile = computed(() => selectedRemoteFiles.value.length === 1 ? selectedRemoteFiles.value[0] : null);
const starred = computed(() => sftpBookmarks.value.includes(sftpPath.value));
const displayedSftpEntries = computed(() => {
  const term = sftpSearch.value.trim().toLocaleLowerCase();
  const entries = sftpEntries.value.filter((entry) => !term || entry.name.toLocaleLowerCase().includes(term));''',
    '''const selectedRemoteFile = computed(() => selectedRemoteFiles.value.length === 1 ? selectedRemoteFiles.value[0] : null);
const starred = computed(() => sftpBookmarks.value.includes(sftpPath.value));
const sftpBreadcrumbs = computed(() => buildRemoteBreadcrumbs(sftpPath.value));
const showHiddenFiles = computed({
  get: () => activeSftpState.value.showHiddenFiles,
  set: (value: boolean) => {
    const state = activeSftpState.value;
    state.showHiddenFiles = value;
    if (!value) {
      state.selectedEntries = state.selectedEntries.filter((entry) => !entry.name.startsWith("."));
      if (state.selectionAnchorPath && !state.selectedEntries.some((entry) => entry.path === state.selectionAnchorPath)) {
        state.selectionAnchorPath = state.selectedEntries.at(-1)?.path;
      }
    }
  },
});
const currentDirectoryTotalSize = computed(() => sftpEntries.value
  .filter((entry) => entry.kind === "file")
  .reduce((total, entry) => total + entry.size, 0));
const runningTransferCount = computed(() => visibleTransfers.value
  .filter((task) => task.state === "running" || task.state === "pausing").length);
const displayedSftpEntries = computed(() => {
  const term = sftpSearch.value.trim().toLocaleLowerCase();
  const entries = sftpEntries.value.filter((entry) => {
    if (!showHiddenFiles.value && entry.name.startsWith(".")) return false;
    return !term || entry.name.toLocaleLowerCase().includes(term);
  });''',
    "navigation computeds",
)

old_load = '''async function loadDirectory(sessionId: string, path: string, recordHistory = true) {
  const session = sessions.value.find((item) => item.id === sessionId);
  const state = ensureSftpSessionState(sftpStates, sessionId);
  if (!session?.connected || !isTauri()) {
    state.entries = [];
    state.error = "请先建立 SSH 连接";
    state.loading = false;
    return;
  }

  const requestVersion = beginSftpDirectoryRequest(state);
  try {
    const listing = await listSftpDirectory(sessionId, path);
    if (!isCurrentSftpDirectoryRequest(sftpStates, state, requestVersion)) return;
    state.path = listing.path;
    state.entries = bindSftpEntries(sessionId, listing.entries);
    if (recordHistory && state.history[state.historyIndex] !== listing.path) {
      state.history = state.history.slice(0, state.historyIndex + 1);
      state.history.push(listing.path);
      state.historyIndex = state.history.length - 1;
      state.recentPaths = [listing.path, ...state.recentPaths.filter((item) => item !== listing.path)].slice(0, 50);
      writeSftpStorage(sessionId, "liteshell.sftp.history.v1", state.recentPaths);
    }
  } catch (error) {
    if (isCurrentSftpDirectoryRequest(sftpStates, state, requestVersion)) {
      state.error = describeCommandError(error);
    }
  } finally {
    finishSftpDirectoryRequest(sftpStates, state, requestVersion);
  }
}

function loadActiveDirectory(path: string, recordHistory = true) {
  const sessionId = activeSessionId.value;
  if (sessionId) void loadDirectory(sessionId, path, recordHistory);
}'''
new_load = '''async function loadDirectory(sessionId: string, path: string, recordHistory = true) {
  const session = sessions.value.find((item) => item.id === sessionId);
  const state = ensureSftpSessionState(sftpStates, sessionId);
  if (!session?.connected || !isTauri()) {
    state.entries = [];
    state.error = "请先建立 SSH 连接";
    state.loading = false;
    return;
  }

  const preserveSelection = !recordHistory && path === state.path;
  const previousSelection = preserveSelection ? [...state.selectedEntries] : [];
  const requestVersion = beginSftpDirectoryRequest(state, preserveSelection);
  try {
    const listing = await listSftpDirectory(sessionId, path);
    if (!isCurrentSftpDirectoryRequest(sftpStates, state, requestVersion)) return;
    state.path = listing.path;
    const entries = bindSftpEntries(sessionId, listing.entries);
    state.entries = entries;
    if (preserveSelection) {
      state.selectedEntries = reconcileSelection(entries, previousSelection);
      if (state.selectionAnchorPath && !entries.some((entry) => entry.path === state.selectionAnchorPath)) {
        state.selectionAnchorPath = state.selectedEntries.at(-1)?.path;
      }
    }
    if (recordHistory && state.history[state.historyIndex] !== listing.path) {
      state.history = state.history.slice(0, state.historyIndex + 1);
      state.history.push(listing.path);
      state.historyIndex = state.history.length - 1;
      state.recentPaths = [listing.path, ...state.recentPaths.filter((item) => item !== listing.path)].slice(0, 50);
      writeSftpStorage(sessionId, "liteshell.sftp.history.v1", state.recentPaths);
    }
  } catch (error) {
    if (isCurrentSftpDirectoryRequest(sftpStates, state, requestVersion)) {
      state.error = describeCommandError(error);
    }
  } finally {
    finishSftpDirectoryRequest(sftpStates, state, requestVersion);
  }
}

async function loadActiveDirectory(path: string, recordHistory = true) {
  const sessionId = activeSessionId.value;
  if (sessionId) await loadDirectory(sessionId, path, recordHistory);
}'''
app = replace_once(app, old_load, new_load, "selection preserving directory load")

app = replace_once(
    app,
    '''function parentPath(path: string) {
  if (path === "/") return "/";
  const normalized = path.replace(/\/+$/, "");
  return normalized.slice(0, normalized.lastIndexOf("/")) || "/";
}

function openSftpEntry(entry: SessionSftpEntry) {
  const state = sftpStates.get(entry.sessionId);
  if (!state || activeSessionId.value !== entry.sessionId) return;
  state.selectedEntries = [entry];
  if (entry.kind === "directory") void loadDirectory(entry.sessionId, entry.path);
}

function selectRemoteEntry(entry: SessionSftpEntry, event: MouseEvent) {
  const state = sftpStates.get(entry.sessionId);
  if (!state || activeSessionId.value !== entry.sessionId) return;
  if (event.ctrlKey || event.metaKey) {
    state.selectedEntries = state.selectedEntries.some((item) => item.path === entry.path)
      ? state.selectedEntries.filter((item) => item.path !== entry.path)
      : [...state.selectedEntries, entry];
  } else {
    state.selectedEntries = [entry];
  }
}''',
    '''function parentPath(path: string) {
  if (path === "/") return "/";
  const normalized = path.replace(/\/+$/, "");
  return normalized.slice(0, normalized.lastIndexOf("/")) || "/";
}

async function beginPathEditing() {
  sftpPathDraft.value = sftpPath.value;
  sftpPathEditing.value = true;
  await nextTick();
  sftpPathInput.value?.focus();
  sftpPathInput.value?.select();
}

function cancelPathEditing() {
  sftpPathDraft.value = sftpPath.value;
  sftpPathEditing.value = false;
}

async function submitPathEditing() {
  const path = sftpPathDraft.value.trim();
  if (!path) return;
  sftpPathEditing.value = false;
  await loadActiveDirectory(path);
  sftpPathDraft.value = sftpPath.value;
}

async function openSftpEntry(entry: SessionSftpEntry) {
  const state = sftpStates.get(entry.sessionId);
  if (!state || activeSessionId.value !== entry.sessionId) return;
  state.selectedEntries = [entry];
  state.selectionAnchorPath = entry.path;
  if (entry.kind === "directory") await loadDirectory(entry.sessionId, entry.path);
  else if (entry.kind === "file" && fileDoubleClickAction.value === "download") await startDownload();
}

function selectRemoteEntry(entry: SessionSftpEntry, event: MouseEvent) {
  const state = sftpStates.get(entry.sessionId);
  if (!state || activeSessionId.value !== entry.sessionId) return;
  const entries = displayedSftpEntries.value;
  const update = updateSelectionPaths(
    entries.map((item) => item.path),
    state.selectedEntries.map((item) => item.path),
    entry.path,
    state.selectionAnchorPath,
    {
      toggle: event.ctrlKey || event.metaKey,
      range: event.shiftKey,
      additiveRange: event.shiftKey && (event.ctrlKey || event.metaKey),
    },
  );
  const selectedPaths = new Set(update.paths);
  state.selectedEntries = entries.filter((item) => selectedPaths.has(item.path));
  state.selectionAnchorPath = update.anchorPath;
}''',
    "path editing and range selection",
)

start = app.index("async function deleteRemoteEntry()")
end = app.index("function formatSize(", start)
replacement = '''async function deleteRemoteEntries() {
  const sessionId = activeSessionId.value;
  const state = sftpStates.get(sessionId);
  const session = sessions.value.find((item) => item.id === sessionId);
  const selected = state ? [...state.selectedEntries] : [];
  if (!state || !session?.connected || !selectionBelongsToSession(selected, sessionId) || !selected.length) return;

  const directoryCount = selected.filter((entry) => entry.kind === "directory").length;
  const fileCount = selected.length - directoryCount;
  const summary = [
    fileCount ? `${fileCount} 个文件或链接` : "",
    directoryCount ? `${directoryCount} 个目录` : "",
  ].filter(Boolean).join("、");
  const confirmed = await ask(
    `确定永久删除已选择的 ${summary} 吗？${directoryCount ? "目录将连同所有子目录和文件递归删除。" : ""}此操作无法撤销。`,
    {
      title: "批量删除远程项目",
      kind: "warning",
      okLabel: `删除 ${selected.length} 项`,
      cancelLabel: "取消",
    },
  );
  if (!confirmed) return;

  const currentPaths = new Set(state.selectedEntries.map((entry) => entry.path));
  if (activeSessionId.value !== sessionId || selected.some((entry) => !currentPaths.has(entry.path))) {
    state.error = "会话或选择已经变化，删除已取消";
    return;
  }

  const failures: string[] = [];
  let deleted = 0;
  for (const entry of selected) {
    try {
      if (entry.kind === "directory") await deleteSftpDirectoryRecursive(sessionId, entry.path);
      else await deleteSftpEntry(sessionId, entry.path, false);
      deleted += 1;
    } catch (error) {
      failures.push(`${entry.name}：${describeCommandError(error)}`);
    }
  }
  await loadDirectory(sessionId, state.path, false);
  state.notice = `已删除 ${deleted} 项${failures.length ? `，${failures.length} 项失败` : ""}`;
  if (failures.length) state.error = failures.slice(0, 3).join("；");
}

function openSftpContextMenu(entry: SessionSftpEntry, event: MouseEvent) {
  const state = sftpStates.get(entry.sessionId);
  if (!state || activeSessionId.value !== entry.sessionId) return;
  if (!state.selectedEntries.some((item) => item.path === entry.path)) {
    state.selectedEntries = [entry];
    state.selectionAnchorPath = entry.path;
  }
  sftpContextMenu.value = {
    x: Math.min(event.clientX, window.innerWidth - 190),
    y: Math.min(event.clientY, window.innerHeight - 230),
    sessionId: entry.sessionId,
    entryPath: entry.path,
  };
}

async function copySelectedRemotePaths() {
  const state = sftpStates.get(activeSessionId.value);
  if (!state?.selectedEntries.length) return;
  try {
    await navigator.clipboard.writeText(state.selectedEntries.map((entry) => entry.path).join("\n"));
    state.notice = `已复制 ${state.selectedEntries.length} 个远程路径`;
  } catch (error) {
    state.error = `复制路径失败：${describeCommandError(error)}`;
  }
}

async function runSftpContextAction(action: "download" | "rename" | "delete" | "copy" | "refresh") {
  const menu = sftpContextMenu.value;
  sftpContextMenu.value = null;
  if (!menu || menu.sessionId !== activeSessionId.value) return;
  const state = sftpStates.get(menu.sessionId);
  if (!state?.selectedEntries.some((entry) => entry.path === menu.entryPath)) return;
  if (action === "download") await startDownload();
  else if (action === "rename") await renameRemoteEntry();
  else if (action === "delete") await deleteRemoteEntries();
  else if (action === "copy") await copySelectedRemotePaths();
  else await loadDirectory(menu.sessionId, state.path, false);
}

'''
app = app[:start] + replacement + app[end:]

old_drop = '''async function handleDroppedPaths(paths: string[]) {
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
}'''
new_drop = '''function isSftpDropPosition(position: { x: number; y: number }) {
  const pane = sftpPaneElement.value;
  if (!pane) return false;
  const scale = window.devicePixelRatio || 1;
  return isPointInsideRect(
    { x: position.x / scale, y: position.y / scale },
    pane.getBoundingClientRect(),
  );
}

async function prepareDroppedPaths(paths: string[]) {
  const session = activeSession.value;
  if (!session?.connected || !paths.length) return;
  const sessionId = session.id;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  selectedTool.value = "files";
  state.error = "";

  const files: string[] = [];
  const directories: DroppedDirectoryPreview[] = [];
  let fileCount = 0;
  let directoryCount = 0;
  let totalSize = 0;
  let skippedCount = 0;
  try {
    for (const path of paths) {
      const inspection = await inspectLocalPath(path);
      if (inspection.kind === "file") {
        files.push(path);
        fileCount += 1;
        totalSize += inspection.size ?? 0;
      } else if (inspection.kind === "directory") {
        const manifest = await scanLocalDirectory(path, sessionId);
        directories.push({ path, manifest });
        fileCount += manifest.fileCount;
        directoryCount += Math.max(1, manifest.directoryCount);
        totalSize += manifest.totalSize;
        skippedCount += manifest.skippedLinks + manifest.skippedUnsupported;
      } else {
        skippedCount += 1;
      }
    }
  } catch (error) {
    state.error = describeCommandError(error);
    return;
  }
  if (!files.length && !directories.length) {
    state.error = "拖放内容中没有可上传的普通文件或目录";
    return;
  }
  sftpDropPreview.value = {
    sessionId,
    serverLabel: session.name,
    targetPath: state.path,
    files,
    directories,
    fileCount,
    directoryCount,
    totalSize,
    skippedCount,
  };
}

async function confirmDroppedPaths() {
  const preview = sftpDropPreview.value;
  if (!preview || sftpDropBusy.value) return;
  const session = sessions.value.find((item) => item.id === preview.sessionId);
  const state = sftpStates.get(preview.sessionId);
  if (!session?.connected || !state || activeSessionId.value !== preview.sessionId || state.path !== preview.targetPath) {
    if (state) state.error = "会话或目标目录已经变化，请重新拖放文件";
    sftpDropPreview.value = null;
    return;
  }

  sftpDropBusy.value = true;
  state.error = "";
  const conflicts = createConflictBatchContext();
  try {
    for (const directory of preview.directories) {
      await uploadDirectoryPath(
        directory.path,
        directory.manifest,
        preview.sessionId,
        conflicts,
        preview.directories.length > 1,
      );
      if (state.error) return;
    }
    if (preview.files.length) await uploadFilePaths(preview.files, preview.sessionId, conflicts);
  } finally {
    sftpDropBusy.value = false;
    sftpDropPreview.value = null;
  }
}'''
app = replace_once(app, old_drop, new_drop, "scoped drop preview")

app = replace_once(
    app,
    '''    unlistenDragDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") sftpDragActive.value = Boolean(activeSession.value?.connected);
      else if (event.payload.type === "drop") {
        sftpDragActive.value = false;
        void handleDroppedPaths(event.payload.paths);
      } else sftpDragActive.value = false;
    });''',
    '''    unlistenDragDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        sftpDragActive.value = Boolean(activeSession.value?.connected)
          && isSftpDropPosition(event.payload.position);
      } else if (event.payload.type === "drop") {
        const insideSftp = isSftpDropPosition(event.payload.position);
        sftpDragActive.value = false;
        if (insideSftp) void prepareDroppedPaths(event.payload.paths);
      } else sftpDragActive.value = false;
    });''',
    "drag drop scope",
)

app = replace_once(
    app,
    '''watch(activeSessionId, (sessionId) => {
  renderActiveTerminal();''',
    '''watch(fileDoubleClickAction, (value) => {
  localStorage.setItem("liteshell.sftp.file-double-click.v1", value);
});

watch(activeSessionId, (sessionId) => {
  sftpContextMenu.value = null;
  sftpPathEditing.value = false;
  sftpPathDraft.value = sessionId ? ensureSftpSessionState(sftpStates, sessionId).path : "";
  renderActiveTerminal();''',
    "settings and session watch",
)

app = replace_once(
    app,
    '''  <div class="app-shell" :class="{ 'sidebar-collapsed': sidebarCollapsed }">''',
    '''  <div class="app-shell" :class="{ 'sidebar-collapsed': sidebarCollapsed }" @click="sftpContextMenu = null">''',
    "root context close",
)

app = replace_once(
    app,
    '''      <section class="sftp-pane">''',
    '''      <section ref="sftpPaneElement" class="sftp-pane">''',
    "sftp pane ref",
)

old_toolbar = '''            <button class="icon-button" aria-label="后退" :disabled="sftpHistoryIndex <= 0" @click="navigateHistory(-1)"><IconArrowLeft :size="20" /></button><button class="icon-button" aria-label="前进" :disabled="sftpHistoryIndex >= sftpHistory.length - 1" @click="navigateHistory(1)"><IconArrowRight :size="20" /></button><button class="icon-button" aria-label="上一级" @click="loadActiveDirectory(parentPath(sftpPath))"><IconArrowUp :size="20" /></button><button class="icon-button" aria-label="刷新" :disabled="sftpLoading" @click="loadActiveDirectory(sftpPath, false)"><IconRefresh :size="19" /></button>
            <div class="path-field"><input v-model="sftpPath" aria-label="远程路径" @keyup.enter="loadActiveDirectory(sftpPath)" /><button class="icon-button" aria-label="收藏路径" @click="toggleSftpBookmark"><component :is="starred ? IconStarFilled : IconStar" :size="20" /></button></div>
            <label class="sftp-search"><IconSearch :size="15" /><input v-model="sftpSearch" placeholder="筛选" /></label>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUpload"><IconUpload :size="18" />上传文件</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUploadDirectory"><IconFolder :size="17" />上传目录</button>
            <button class="toolbar-button" :disabled="!selectedRemoteFiles.length" @click="startDownload"><IconArrowDown :size="18" />下载{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="createRemoteDirectory"><IconPlus :size="18" />新建目录</button>
            <button class="toolbar-button" :disabled="selectedRemoteFiles.length !== 1" @click="renameRemoteEntry">重命名</button>
            <button class="toolbar-button danger" :disabled="selectedRemoteFiles.length !== 1" @click="deleteRemoteEntry"><IconTrash :size="16" />删除</button>'''
new_toolbar = '''            <button class="icon-button" aria-label="后退" :disabled="sftpHistoryIndex <= 0" @click="navigateHistory(-1)"><IconArrowLeft :size="20" /></button><button class="icon-button" aria-label="前进" :disabled="sftpHistoryIndex >= sftpHistory.length - 1" @click="navigateHistory(1)"><IconArrowRight :size="20" /></button><button class="icon-button" aria-label="上一级" @click="loadActiveDirectory(parentPath(sftpPath))"><IconArrowUp :size="20" /></button><button class="icon-button" aria-label="刷新" :disabled="sftpLoading" @click="loadActiveDirectory(sftpPath, false)"><IconRefresh :size="19" /></button>
            <div class="path-field">
              <input v-if="sftpPathEditing" ref="sftpPathInput" v-model="sftpPathDraft" aria-label="编辑远程路径" @keyup.enter="submitPathEditing" @keyup.esc="cancelPathEditing" />
              <nav v-else class="path-breadcrumbs" aria-label="远程路径面包屑"><button v-for="crumb in sftpBreadcrumbs" :key="crumb.path" @click="loadActiveDirectory(crumb.path)">{{ crumb.label }}</button></nav>
              <button class="path-edit-button" :aria-label="sftpPathEditing ? '取消编辑路径' : '编辑远程路径'" @click="sftpPathEditing ? cancelPathEditing() : beginPathEditing()">{{ sftpPathEditing ? '取消' : '编辑' }}</button>
              <button class="icon-button" aria-label="收藏路径" @click="toggleSftpBookmark"><component :is="starred ? IconStarFilled : IconStar" :size="20" /></button>
            </div>
            <label class="sftp-search"><IconSearch :size="15" /><input v-model="sftpSearch" placeholder="筛选" /></label>
            <label class="sftp-option"><input v-model="showHiddenFiles" type="checkbox" />隐藏文件</label>
            <label class="sftp-option">双击文件<select v-model="fileDoubleClickAction"><option value="select">仅选择</option><option value="download">下载</option></select></label>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUpload"><IconUpload :size="18" />上传文件</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUploadDirectory"><IconFolder :size="17" />上传目录</button>
            <button class="toolbar-button" :disabled="!selectedRemoteFiles.length" @click="startDownload"><IconArrowDown :size="18" />下载{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="createRemoteDirectory"><IconPlus :size="18" />新建目录</button>
            <button class="toolbar-button" :disabled="selectedRemoteFiles.length !== 1" @click="renameRemoteEntry">重命名</button>
            <button class="toolbar-button danger" :disabled="!selectedRemoteFiles.length" @click="deleteRemoteEntries"><IconTrash :size="16" />删除{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>'''
app = replace_once(app, old_toolbar, new_toolbar, "toolbar navigation controls")

app = replace_once(
    app,
    '''            <button v-for="file in displayedSftpEntries" :key="file.path" class="file-row" :class="{ selected: selectedRemoteFiles.some(item => item.path === file.path) }" role="row" @click="selectRemoteEntry(file, $event)" @dblclick="openSftpEntry(file)"><span><component :is="file.kind === 'directory' ? IconFolder : IconFile" :size="21" :class="file.kind === 'directory' ? 'folder' : 'file'" />{{ file.name }}</span><span>{{ formatSize(file.size, file.kind) }}</span><span>{{ formatModified(file.modifiedAt) }}</span><span>{{ file.permissions }}</span></button>''',
    '''            <button v-for="file in displayedSftpEntries" :key="file.path" class="file-row" :class="{ selected: selectedRemoteFiles.some(item => item.path === file.path) }" role="row" @click="selectRemoteEntry(file, $event)" @dblclick="openSftpEntry(file)" @contextmenu.prevent="openSftpContextMenu(file, $event)"><span><component :is="file.kind === 'directory' ? IconFolder : IconFile" :size="21" :class="file.kind === 'directory' ? 'folder' : 'file'" />{{ file.name }}</span><span>{{ formatSize(file.size, file.kind) }}</span><span>{{ formatModified(file.modifiedAt) }}</span><span>{{ file.permissions }}</span></button>''',
    "file context menu",
)

app = replace_once(
    app,
    '''          <footer class="file-summary">{{ sftpEntries.length }} 项<span v-if="selectedRemoteFiles.length"> · 已选择 {{ selectedRemoteFiles.length }} 项</span></footer>''',
    '''          <footer class="file-summary">{{ sftpEntries.length }} 项 · {{ formatSize(currentDirectoryTotalSize, 'file') }}<span v-if="selectedRemoteFiles.length"> · 已选择 {{ selectedRemoteFiles.length }} 项</span></footer>''',
    "file summary",
)

app = replace_once(
    app,
    '''    <footer class="statusbar"><span><i class="online-dot" :class="{ offline: !activeSession?.connected }"></i>{{ activeSession?.connected ? '已连接' : '未连接' }}</span><span class="latency">{{ systemMetrics ? `${systemMetrics.latencyMs} ms` : '--' }}</span><span>UTF-8</span><span>转发 0</span></footer>''',
    '''    <footer class="statusbar"><span><i class="online-dot" :class="{ offline: !activeSession?.connected }"></i>{{ activeSession?.connected ? '已连接' : '未连接' }}</span><span>SFTP {{ sftpEntries.length }} 项 / 已选 {{ selectedRemoteFiles.length }} / {{ formatSize(currentDirectoryTotalSize, 'file') }}</span><span>运行传输 {{ runningTransferCount }}</span><span class="latency">{{ systemMetrics ? `${systemMetrics.latencyMs} ms` : '--' }}</span><span>UTF-8</span></footer>''',
    "status bar summary",
)

app = replace_once(
    app,
    '''    <ConnectionManager :open="showConnectionManager" @close="showConnectionManager = false" @changed="profiles = $event.profiles" @connect="connectManagedProfiles" />''',
    '''    <div v-if="sftpContextMenu" class="sftp-context-menu" :style="{ left: `${sftpContextMenu.x}px`, top: `${sftpContextMenu.y}px` }" @click.stop>
      <button @click="runSftpContextAction('download')">下载所选项目</button>
      <button :disabled="selectedRemoteFiles.length !== 1" @click="runSftpContextAction('rename')">重命名</button>
      <button class="danger" @click="runSftpContextAction('delete')">删除所选项目</button>
      <button @click="runSftpContextAction('copy')">复制远程路径</button>
      <button @click="runSftpContextAction('refresh')">刷新当前目录</button>
    </div>
    <div v-if="sftpDropPreview" class="dialog-backdrop drop-preview-backdrop">
      <section class="drop-preview-dialog" role="dialog" aria-modal="true" aria-label="确认拖放上传">
        <header><strong>确认拖放上传</strong></header>
        <dl><div><dt>服务器</dt><dd>{{ sftpDropPreview.serverLabel }}</dd></div><div><dt>目标目录</dt><dd>{{ sftpDropPreview.targetPath }}</dd></div><div><dt>文件</dt><dd>{{ sftpDropPreview.fileCount }} 个</dd></div><div><dt>目录</dt><dd>{{ sftpDropPreview.directoryCount }} 个</dd></div><div><dt>总大小</dt><dd>{{ formatSize(sftpDropPreview.totalSize, 'file') }}</dd></div><div v-if="sftpDropPreview.skippedCount"><dt>跳过</dt><dd>{{ sftpDropPreview.skippedCount }} 个链接或不支持项</dd></div></dl>
        <footer><button class="secondary-button" :disabled="sftpDropBusy" @click="sftpDropPreview = null">取消</button><button class="primary-button" :disabled="sftpDropBusy" @click="confirmDroppedPaths">{{ sftpDropBusy ? '上传中…' : '开始上传' }}</button></footer>
      </section>
    </div>
    <ConnectionManager :open="showConnectionManager" @close="showConnectionManager = false" @changed="profiles = $event.profiles" @connect="connectManagedProfiles" />''',
    "context and drop preview dialogs",
)

app_path.write_text(app, encoding="utf-8")

service_path = Path("src/services/ssh.ts")
service = service_path.read_text(encoding="utf-8")
service = replace_once(
    service,
    '''export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";''',
    '''export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";
export type LocalPathInspection = { kind: LocalPathKind; size?: number };''',
    "local inspection type",
)
service = replace_once(
    service,
    '''export const inspectLocalPath = (path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_local_path", { path });''',
    '''export const inspectLocalPath = (path: string) =>
  invoke<LocalPathInspection>("sftp_inspect_local_path", { path });''',
    "local inspection api",
)
service_path.write_text(service, encoding="utf-8")

rust_path = Path("src-tauri/src/sftp_directory.rs")
rust = rust_path.read_text(encoding="utf-8")
rust = replace_once(
    rust,
    '''pub struct LocalPathInspection {
    kind: &'static str,
}''',
    '''pub struct LocalPathInspection {
    kind: &'static str,
    size: Option<u64>,
}''',
    "rust local inspection type",
)
rust = replace_once(
    rust,
    '''        Ok(metadata) if is_local_link_or_reparse(&metadata) => {
            Ok(LocalPathInspection { kind: "other" })
        }
        Ok(metadata) if metadata.is_dir() => Ok(LocalPathInspection { kind: "directory" }),
        Ok(metadata) if metadata.is_file() => Ok(LocalPathInspection { kind: "file" }),
        Ok(_) => Ok(LocalPathInspection { kind: "other" }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(LocalPathInspection { kind: "missing" })
        }''',
    '''        Ok(metadata) if is_local_link_or_reparse(&metadata) => Ok(LocalPathInspection {
            kind: "other",
            size: None,
        }),
        Ok(metadata) if metadata.is_dir() => Ok(LocalPathInspection {
            kind: "directory",
            size: None,
        }),
        Ok(metadata) if metadata.is_file() => Ok(LocalPathInspection {
            kind: "file",
            size: Some(metadata.len()),
        }),
        Ok(_) => Ok(LocalPathInspection {
            kind: "other",
            size: None,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LocalPathInspection {
            kind: "missing",
            size: None,
        }),''',
    "rust local inspection values",
)
rust_path.write_text(rust, encoding="utf-8")

styles_path = Path("src/styles.css")
styles = styles_path.read_text(encoding="utf-8")
styles = replace_once(
    styles,
    '''.path-field span { margin-right: auto; color: #b9c6cc; }
.path-field input { min-width: 0; height: 100%; flex: 1; border: 0; outline: 0; color: #b9c6cc; background: transparent; }
.path-field button { width: 34px; height: 32px; }''',
    '''.path-field span { margin-right: auto; color: #b9c6cc; }
.path-field input { min-width: 0; height: 100%; flex: 1; border: 0; outline: 0; color: #b9c6cc; background: transparent; }
.path-field > button { min-width: 34px; height: 32px; }
.path-breadcrumbs { min-width: 0; display: flex; align-items: center; flex: 1; overflow: auto; scrollbar-width: none; }
.path-breadcrumbs button { height: 30px; flex: none; padding: 0 7px; border: 0; color: #b9c6cc; background: transparent; cursor: pointer; }
.path-breadcrumbs button + button::before { content: "/"; margin-right: 9px; color: #607884; }
.path-breadcrumbs button:hover { color: #e2edf2; background: #1b3440; }
.path-edit-button { padding: 0 7px; border: 0; color: #79bfe8; background: transparent; cursor: pointer; font-size: 10px; }''',
    "breadcrumb styles",
)
styles = replace_once(
    styles,
    '''.sftp-search input { min-width: 0; width: 100%; border: 0; outline: 0; color: #c8d4da; background: transparent; font-size: 11px; }
.toolbar-button { height: 34px;''',
    '''.sftp-search input { min-width: 0; width: 100%; border: 0; outline: 0; color: #c8d4da; background: transparent; font-size: 11px; }
.sftp-option { height: 32px; display: inline-flex; align-items: center; gap: 5px; color: #8fa3ad; white-space: nowrap; font-size: 10px; }
.sftp-option select { height: 25px; border: 1px solid #304955; border-radius: 3px; color: #b9c7ce; background: #102630; font-size: 10px; }
.toolbar-button { height: 34px;''',
    "toolbar option styles",
)
styles += '''

.sftp-context-menu { position: fixed; z-index: 140; width: 184px; display: grid; padding: 5px; border: 1px solid #405965; border-radius: 4px; background: #142c38; box-shadow: 0 12px 32px rgba(0,0,0,.42); }
.sftp-context-menu button { height: 31px; padding: 0 10px; border: 0; border-radius: 2px; color: #c9d5da; background: transparent; text-align: left; cursor: pointer; }
.sftp-context-menu button:hover:not(:disabled) { background: #21404d; }
.sftp-context-menu button:disabled { color: #627681; cursor: default; }
.sftp-context-menu button.danger { color: #ffafb5; }
.drop-preview-backdrop { z-index: 130; }
.drop-preview-dialog { width: min(480px, 100%); padding: 18px; border: 1px solid #405965; border-radius: 5px; color: #dce5ea; background: #142c38; box-shadow: 0 16px 45px rgba(0,0,0,.45); }
.drop-preview-dialog header { margin-bottom: 13px; font-size: 15px; }
.drop-preview-dialog dl { margin: 0; padding: 8px 12px; border: 1px solid #304a57; border-radius: 3px; background: #0e222c; }
.drop-preview-dialog dl div { display: grid; grid-template-columns: 92px minmax(0, 1fr); gap: 10px; padding: 6px 0; }
.drop-preview-dialog dt { color: #8297a2; }
.drop-preview-dialog dd { min-width: 0; margin: 0; overflow-wrap: anywhere; color: #d0dbe0; }
.drop-preview-dialog footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
'''
styles_path.write_text(styles, encoding="utf-8")
