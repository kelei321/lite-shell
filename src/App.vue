<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import "@xterm/xterm/css/xterm.css";
import ConnectionManager from "./components/ConnectionManager.vue";
import SftpDirectoryTree from "./components/sftp/SftpDirectoryTree.vue";
import {
  cancelSftpTransfer,
  commandErrorCode,
  connectProfile,
  connectSsh,
  createSftpDirectory,
  deleteSftpEntry,
  deleteSftpDirectoryRecursive,
  deleteProfile,
  describeCommandError,
  fetchSystemMetrics,
  finishDirectoryReplacement,
  getLocalDirectoryManifest,
  getRemoteDirectoryManifest,
  disconnectSsh,
  inspectLocalPath,
  inspectRemotePath,
  isTauri,
  listProfiles,
  listSftpDirectories,
  listSftpDirectory,
  prepareLocalDirectory,
  prepareRemoteDirectory,
  listenSftpQueueTasks,
  listenSshEvents,
  resizeSsh,
  renameSftpEntry,
  saveProfile,
  sendSshInput,
  wakeSftpTransferQueue,
  type ConnectionProfile,
  type ConflictStrategy,
  type DirectoryConflictStrategy,
  type DirectoryPrepareResult,
  type LocalDirectoryManifest,
  type RecursiveScanSummary,
  type RemoteDirectoryManifest,
  type ConnectRequest,
  type SftpEntry,
  type SshEvent,
  type SystemMetrics,
  type TransferQueueTask,
} from "./services/ssh";
import {
  beginSftpDirectoryRequest,
  bindSftpEntries,
  createSftpSessionState,
  ensureSftpSessionState,
  finishSftpDirectoryRequest,
  isCurrentSftpDirectoryRequest,
  removeSftpSessionState,
  selectionBelongsToSession,
  type SessionSftpEntry,
  type SftpSessionState,
} from "./sftp/session-state";
import { useSftpTransferQueue } from "./sftp/transfer-queue";
import {
  applyDirectoryTreeListing,
  beginDirectoryTreeRequest,
  ensureDirectoryTreeNode,
  ensureSftpDirectoryTreeState,
  failDirectoryTreeRequest,
  finishDirectoryTreeRequest,
  removeSftpDirectoryTreeState,
  selectDirectoryTreePath,
  type SftpDirectoryTreeState,
} from "./sftp/directory-tree-state";
import {
  buildRemoteBreadcrumbs,
  isPointInsideRect,
  reconcileSelection,
  selectionsMatchSnapshot,
  updateSelectionPaths,
} from "./sftp/navigation-state";
import {
  IconAdjustmentsHorizontal,
  IconArrowDown,
  IconArrowLeft,
  IconArrowRight,
  IconArrowUp,
  IconBookmark,
  IconChevronDown,
  IconChevronLeft,
  IconChevronRight,
  IconChevronUp,
  IconClockHour4,
  IconFile,
  IconFolder,
  IconMenu2,
  IconPlus,
  IconRefresh,
  IconSearch,
  IconServer,
  IconShieldCheck,
  IconSquarePlus,
  IconStar,
  IconStarFilled,
  IconTrash,
  IconUpload,
  IconX,
} from "@tabler/icons-vue";

type Session = { id: string; name: string; connected: boolean; state?: SshEvent["kind"]; profileId?: string };

const sessions = ref<Session[]>([]);
const activeSessionId = ref("");
const search = ref("");
const statusExpanded = ref(true);
const sidebarCollapsed = ref(false);
const selectedTool = ref<"files" | "bookmarks" | "history">("files");
const directoryTreeStates = reactive(new Map<string, SftpDirectoryTreeState>());
const emptyDirectoryTreeState = reactive(ensureSftpDirectoryTreeState(new Map(), ""));
const activeDirectoryTreeState = computed(() => activeSessionId.value
  ? ensureSftpDirectoryTreeState(directoryTreeStates, activeSessionId.value)
  : emptyDirectoryTreeState);
const storedSftpTreeWidth = Number(localStorage.getItem("liteshell.sftp.tree-width.v1"));
const sftpTreeWidth = ref(Number.isFinite(storedSftpTreeWidth)
  ? Math.min(420, Math.max(160, storedSftpTreeWidth))
  : 224);
const sftpTreeCollapsed = ref(false);
const sftpStates = reactive(new Map<string, SftpSessionState>());
const emptySftpState = reactive(createSftpSessionState(""));
const activeSftpState = computed(() => activeSessionId.value
  ? ensureSftpSessionState(sftpStates, activeSessionId.value)
  : emptySftpState);
const sftpPath = computed({
  get: () => activeSftpState.value.path,
  set: (value: string) => { activeSftpState.value.path = value; },
});
const sftpEntries = computed({
  get: () => activeSftpState.value.entries,
  set: (value: SessionSftpEntry[]) => { activeSftpState.value.entries = value; },
});
const sftpLoading = computed({
  get: () => activeSftpState.value.loading,
  set: (value: boolean) => { activeSftpState.value.loading = value; },
});
const sftpError = computed({
  get: () => activeSftpState.value.error,
  set: (value: string) => { activeSftpState.value.error = value; },
});
const selectedRemoteFiles = computed({
  get: () => activeSftpState.value.selectedEntries,
  set: (value: SessionSftpEntry[]) => { activeSftpState.value.selectedEntries = value; },
});
const {
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
const recursiveScan = ref<{ id: string; sessionId: string; label: string } | null>(null);
const sftpHistory = computed({
  get: () => activeSftpState.value.history,
  set: (value: string[]) => { activeSftpState.value.history = value; },
});
const sftpHistoryIndex = computed({
  get: () => activeSftpState.value.historyIndex,
  set: (value: number) => { activeSftpState.value.historyIndex = value; },
});
const sftpBookmarks = computed({
  get: () => activeSftpState.value.bookmarks,
  set: (value: string[]) => { activeSftpState.value.bookmarks = value; },
});
const sftpRecentPaths = computed({
  get: () => activeSftpState.value.recentPaths,
  set: (value: string[]) => { activeSftpState.value.recentPaths = value; },
});
const sftpSearch = ref("");
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
const sftpDropBusy = ref(false);
type ConflictKind = "file" | "directory";
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
const refreshedAt = ref("刚刚");
const systemMetrics = ref<SystemMetrics | null>(null);
const monitorError = ref("");
const monitorBusy = ref(false);
const networkRxHistory = ref<number[]>(Array(32).fill(0));
const networkTxHistory = ref<number[]>(Array(32).fill(0));
const chartCanvas = ref<HTMLCanvasElement | null>(null);
const terminalContainer = ref<HTMLDivElement | null>(null);
const showConnectDialog = ref(false);
const showConnectionManager = ref(false);
const connectBusy = ref(false);
const connectError = ref("");
const profiles = ref<ConnectionProfile[]>([]);
const pendingFingerprint = ref<{ fingerprint: string; algorithm: string; sessionId: string; profileId?: string } | null>(null);
const connectionForm = ref({
  profileId: "",
  name: "",
  group: "默认分组",
  folderId: "folder-default",
  host: "127.0.0.1",
  port: 22,
  username: "root",
  authType: "password" as "password" | "private_key",
  password: "",
  privateKeyPath: "",
  passphrase: "",
  favorite: false,
  rememberProfile: true,
  rememberSecret: true,
});

let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let terminalResizeObserver: ResizeObserver | null = null;
let unlistenSsh: (() => void) | null = null;
let unlistenSftp: (() => void) | null = null;
let unlistenDragDrop: (() => void) | null = null;
let stopSftpTreeResize: (() => void) | null = null;
let monitorTimer: number | null = null;
const terminalBuffers = new Map<string, string>();
const textDecoder = new TextDecoder();
const visibleProfiles = computed(() => {
  const term = search.value.trim().toLowerCase();
  return profiles.value.filter((profile) =>
    !term || [profile.name, profile.host, profile.username, profile.group].some((value) => value.toLowerCase().includes(term)),
  );
});
const cpuPercent = computed(() => systemMetrics.value?.cpuUsagePercent ?? 0);
const memoryPercent = computed(() => percentage(systemMetrics.value?.memoryUsed, systemMetrics.value?.memoryTotal));
const swapPercent = computed(() => percentage(systemMetrics.value?.swapUsed, systemMetrics.value?.swapTotal));
const networkScale = computed(() => Math.max(1024, ...networkRxHistory.value, ...networkTxHistory.value));
const activeSession = computed(() => sessions.value.find((session) => session.id === activeSessionId.value));
const selectedRemoteFile = computed(() => selectedRemoteFiles.value.length === 1 ? selectedRemoteFiles.value[0] : null);
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
  });
  return [...entries].sort((left, right) => {
    if (left.kind === "directory" && right.kind !== "directory") return -1;
    if (left.kind !== "directory" && right.kind === "directory") return 1;
    let result = 0;
    if (sftpSortKey.value === "name") result = left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: "base" });
    else result = (left[sftpSortKey.value] ?? 0) - (right[sftpSortKey.value] ?? 0);
    return result * (sftpSortAscending.value ? 1 : -1);
  });
});

function changeSftpSort(key: typeof sftpSortKey.value) {
  if (sftpSortKey.value === key) sftpSortAscending.value = !sftpSortAscending.value;
  else {
    sftpSortKey.value = key;
    sftpSortAscending.value = true;
  }
}

function createConflictBatchContext(): ConflictBatchContext {
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
      `替换目录“${request.name}”会删除目标中源目录不存在的额外内容。新内容会先写入独立临时目录，提交时再安全备份并替换原目录；复制失败不会改动原目录。确定继续吗？`,
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

function sftpStorageScope(sessionId: string) {
  const session = sessions.value.find((item) => item.id === sessionId);
  return session?.profileId ?? session?.name ?? sessionId ?? "default";
}

function readSftpStorage(sessionId: string, key: string) {
  try {
    const all = JSON.parse(localStorage.getItem(key) ?? "{}") as Record<string, string[]>;
    return all[sftpStorageScope(sessionId)] ?? [];
  } catch {
    return [];
  }
}

function writeSftpStorage(sessionId: string, key: string, values: string[]) {
  if (!sessionId) return;
  try {
    const all = JSON.parse(localStorage.getItem(key) ?? "{}") as Record<string, string[]>;
    all[sftpStorageScope(sessionId)] = values;
    localStorage.setItem(key, JSON.stringify(all));
  } catch {
    // 存储不可用时仍允许当前会话继续使用 SFTP。
  }
}

function loadSftpStorage(sessionId: string) {
  if (!sessionId) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  state.bookmarks = readSftpStorage(sessionId, "liteshell.sftp.bookmarks.v1");
  state.recentPaths = readSftpStorage(sessionId, "liteshell.sftp.history.v1");
}

function toggleSftpBookmark() {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  state.bookmarks = state.bookmarks.includes(state.path)
    ? state.bookmarks.filter((item) => item !== state.path)
    : [state.path, ...state.bookmarks.filter((item) => item !== state.path)].slice(0, 50);
  writeSftpStorage(sessionId, "liteshell.sftp.bookmarks.v1", state.bookmarks);
}

function removeSftpBookmark(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  state.bookmarks = state.bookmarks.filter((item) => item !== path);
  writeSftpStorage(sessionId, "liteshell.sftp.bookmarks.v1", state.bookmarks);
}

function clearSftpHistory() {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  state.recentPaths = [];
  writeSftpStorage(sessionId, "liteshell.sftp.history.v1", []);
}

function openProfile(profile: ConnectionProfile) {
  connectError.value = "";
  pendingFingerprint.value = null;
  connectionForm.value = {
    profileId: profile.id,
    name: profile.name,
    group: profile.group,
    folderId: profile.folderId,
    host: profile.host,
    port: profile.port,
    username: profile.username,
    authType: profile.authType,
    password: "",
    privateKeyPath: profile.privateKeyPath ?? "",
    passphrase: "",
    favorite: profile.favorite,
    rememberProfile: true,
    rememberSecret: profile.hasSecret,
  };
  showConnectDialog.value = true;
}

async function removeProfile(profile: ConnectionProfile) {
  if (!isTauri() || !window.confirm(`确定删除连接“${profile.name}”吗？保存的凭据也会一并删除。`)) return;
  await deleteProfile(profile.id);
  profiles.value = profiles.value.filter((item) => item.id !== profile.id);
}

async function closeSession(id: string) {
  const index = sessions.value.findIndex((session) => session.id === id);
  sessions.value = sessions.value.filter((session) => session.id !== id);
  terminalBuffers.delete(id);
  removeSftpSessionState(sftpStates, id);
  removeSftpDirectoryTreeState(directoryTreeStates, id);
  if (isTauri()) await disconnectSsh(id).catch(() => undefined);
  if (activeSessionId.value === id) {
    activeSessionId.value = sessions.value[Math.max(0, index - 1)]?.id ?? "";
  }
}

function addSession() {
  connectError.value = "";
  pendingFingerprint.value = null;
  connectionForm.value.profileId = "";
  connectionForm.value.name = "";
  connectionForm.value.password = "";
  connectionForm.value.passphrase = "";
  showConnectDialog.value = true;
}

function buildConnectRequest(expectedHostFingerprint?: string): ConnectRequest {
  const form = connectionForm.value;
  const id = pendingFingerprint.value?.sessionId ?? crypto.randomUUID();
  return {
    sessionId: id,
    host: form.host.trim(),
    port: Number(form.port),
    username: form.username.trim(),
    auth: form.authType === "password"
      ? { type: "password", password: form.password }
      : {
          type: "private_key",
          path: form.privateKeyPath.trim(),
          passphrase: form.passphrase || undefined,
        },
    cols: terminal?.cols ?? 120,
    rows: terminal?.rows ?? 36,
    expectedHostFingerprint,
  };
}

async function submitConnection(expectedFingerprint?: string) {
  if (!isTauri()) {
    connectError.value = "请使用 Tauri 桌面端运行真实 SSH 连接";
    return;
  }
  connectBusy.value = true;
  connectError.value = "";
  const request = buildConnectRequest(expectedFingerprint);
  if (!request.host || !request.username || !request.port) {
    connectBusy.value = false;
    connectError.value = "请填写主机、端口和用户名";
    return;
  }
  if (request.auth.type === "private_key" && !request.auth.path) {
    connectBusy.value = false;
    connectError.value = "请选择或输入私钥路径";
    return;
  }

  const existing = sessions.value.find((session) => session.id === request.sessionId);
  if (!existing) sessions.value.push({ id: request.sessionId, name: request.host, connected: false, state: "connecting" });
  activeSessionId.value = request.sessionId;

  try {
    let profileId = pendingFingerprint.value?.profileId;
    if (!expectedFingerprint && connectionForm.value.rememberProfile) {
      const secret = connectionForm.value.authType === "password"
        ? connectionForm.value.password
        : connectionForm.value.passphrase;
      const saved = await saveProfile({
        id: connectionForm.value.profileId || undefined,
        name: connectionForm.value.name.trim() || request.host,
        host: request.host,
        port: request.port,
        username: request.username,
        authType: connectionForm.value.authType,
        privateKeyPath: connectionForm.value.privateKeyPath.trim() || undefined,
        group: connectionForm.value.group.trim() || "默认分组",
        folderId: connectionForm.value.folderId,
        favorite: connectionForm.value.favorite,
        secret: secret || undefined,
        rememberSecret: connectionForm.value.rememberSecret,
      });
      connectionForm.value.profileId = saved.id;
      profileId = saved.id;
      profiles.value = await listProfiles();
    }

    const savedProfile = profileId ? profiles.value.find((profile) => profile.id === profileId) : undefined;
    const canUseSavedCredential = savedProfile
      && (savedProfile.authType === "private_key" || savedProfile.hasSecret);
    const outcome = canUseSavedCredential
      ? await connectProfile({
          profileId: savedProfile.id,
          sessionId: request.sessionId,
          cols: request.cols,
          rows: request.rows,
          expectedHostFingerprint: expectedFingerprint,
        })
      : await connectSsh(request);
    if (outcome.status === "host_key_confirmation_required") {
      pendingFingerprint.value = { ...outcome, sessionId: request.sessionId, profileId };
      return;
    }
    const session = sessions.value.find((item) => item.id === request.sessionId);
    if (session) {
      session.connected = true;
      session.state = "connected";
      session.name = request.host;
      session.profileId = profileId;
    }
    terminalBuffers.set(request.sessionId, "");
    renderActiveTerminal();
    connectionForm.value.password = "";
    connectionForm.value.passphrase = "";
    showConnectDialog.value = false;
    pendingFingerprint.value = null;
    void loadDirectory(request.sessionId, ".");
  } catch (error) {
    connectError.value = describeCommandError(error);
    const session = sessions.value.find((item) => item.id === request.sessionId);
    if (session) session.state = "error";
    const message = `\r\n\x1b[31m${connectError.value}\x1b[0m\r\n`;
    terminalBuffers.set(request.sessionId, message);
    if (activeSessionId.value === request.sessionId) renderActiveTerminal();
  } finally {
    connectBusy.value = false;
  }
}

async function connectManagedProfiles(items: ConnectionProfile[]) {
  showConnectionManager.value = false;
  const missingCredential = items.find((profile) => profile.authType === "password" && !profile.hasSecret);
  if (missingCredential) {
    openProfile(missingCredential);
    connectError.value = "部分连接没有保存密码，请先补充凭据；其余可用连接将继续打开";
  }
  const queue = items.filter((profile) => profile.authType !== "password" || profile.hasSecret);
  const workers = Array.from({ length: Math.min(3, queue.length) }, async () => {
    while (queue.length) {
      const profile = queue.shift();
      if (!profile) return;
      const existing = sessions.value.find((session) => session.profileId === profile.id && session.connected);
      if (existing) {
        activeSessionId.value = existing.id;
        continue;
      }
      const sessionId = crypto.randomUUID();
      sessions.value.push({ id: sessionId, name: profile.name, connected: false, state: "connecting", profileId: profile.id });
      activeSessionId.value = sessionId;
      try {
        const outcome = await connectProfile({
          profileId: profile.id,
          sessionId,
          cols: terminal?.cols ?? 120,
          rows: terminal?.rows ?? 36,
        });
        if (outcome.status === "host_key_confirmation_required") {
          openProfile(profile);
          pendingFingerprint.value = { ...outcome, sessionId, profileId: profile.id };
          continue;
        }
        const session = sessions.value.find((item) => item.id === sessionId);
        if (session) {
          session.connected = true;
          session.state = "connected";
        }
        terminalBuffers.set(sessionId, "");
        void loadDirectory(sessionId, ".");
      } catch (cause) {
        const session = sessions.value.find((item) => item.id === sessionId);
        if (session) session.state = "error";
        terminalBuffers.set(sessionId, `\r\n\x1b[31m${describeCommandError(cause)}\x1b[0m\r\n`);
      }
    }
  });
  await Promise.all(workers);
  renderActiveTerminal();
}

function confirmHostKey() {
  const pending = pendingFingerprint.value;
  if (pending) void submitConnection(pending.fingerprint);
}

function decodeBase64(value: string) {
  const binary = atob(value);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return textDecoder.decode(bytes, { stream: true });
}

function handleSshEvent(event: SshEvent) {
  const session = sessions.value.find((item) => item.id === event.sessionId);
  if (session) {
    session.state = event.kind;
    session.connected = event.kind === "connected" || (session.connected && event.kind === "data");
    if (event.kind === "disconnected" || event.kind === "exit" || event.kind === "error") session.connected = false;
  }
  if (["connected", "disconnected", "exit", "error"].includes(event.kind)) {
    void wakeSftpTransferQueue().catch(() => undefined);
    void refreshTransferQueue().catch(() => undefined);
  }
  if (event.kind === "connected" && event.sessionId === activeSessionId.value) void refreshMetrics();

  if (event.kind === "data" && event.dataBase64) {
    const output = decodeBase64(event.dataBase64);
    const buffer = `${terminalBuffers.get(event.sessionId) ?? ""}${output}`.slice(-1_000_000);
    terminalBuffers.set(event.sessionId, buffer);
    if (event.sessionId === activeSessionId.value) terminal?.write(output);
  } else if (event.kind === "error" && event.message) {
    const message = `\r\n\x1b[31m${event.message}\x1b[0m\r\n`;
    terminalBuffers.set(event.sessionId, `${terminalBuffers.get(event.sessionId) ?? ""}${message}`);
    if (event.sessionId === activeSessionId.value) terminal?.write(message);
  } else if (event.kind === "disconnected" && event.sessionId === activeSessionId.value) {
    terminal?.write("\r\n\x1b[90m[会话已断开]\x1b[0m\r\n");
  }
}

function renderActiveTerminal() {
  if (!terminal) return;
  terminal.reset();
  const buffer = terminalBuffers.get(activeSessionId.value) ?? "";
  if (buffer) terminal.write(buffer);
  fitAddon?.fit();
}

async function loadDirectoryTreeNode(sessionId: string, path: string, force = false) {
  const session = sessions.value.find((item) => item.id === sessionId);
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const node = ensureDirectoryTreeNode(treeState, path);
  if (!session?.connected || !isTauri()) {
    node.loading = false;
    node.error = "请先建立 SSH 连接";
    return;
  }
  if (node.loaded && !force && !node.error) return;

  const requestVersion = beginDirectoryTreeRequest(treeState, node.path);
  try {
    const listing = await listSftpDirectories(sessionId, node.path);
    applyDirectoryTreeListing(
      treeState,
      node.path,
      requestVersion,
      listing.path,
      listing.directories,
    );
  } catch (error) {
    failDirectoryTreeRequest(
      treeState,
      node.path,
      requestVersion,
      describeCommandError(error),
    );
  } finally {
    finishDirectoryTreeRequest(treeState, node.path, requestVersion);
  }
}

async function synchronizeDirectoryTreePath(sessionId: string, path: string) {
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const ancestors = selectDirectoryTreePath(treeState, path);
  const ancestorsToLoad = ancestors.length === 1 ? ancestors : ancestors.slice(0, -1);
  for (const ancestor of ancestorsToLoad) {
    const node = ensureDirectoryTreeNode(treeState, ancestor);
    node.expanded = true;
    if (!node.loaded && !node.loading) await loadDirectoryTreeNode(sessionId, ancestor);
  }
}

async function toggleDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const node = ensureDirectoryTreeNode(treeState, path);
  if (node.expanded && node.loaded && !node.error) {
    node.expanded = false;
    return;
  }
  node.expanded = true;
  if (!node.loaded || node.error) await loadDirectoryTreeNode(sessionId, path);
}

async function openDirectoryTreePath(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  selectedTool.value = "files";
  await loadDirectory(sessionId, path);
}

function refreshDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  ensureDirectoryTreeNode(treeState, path).expanded = true;
  void loadDirectoryTreeNode(sessionId, path, true);
}

function beginSftpTreeResize(event: PointerEvent) {
  if (sftpTreeCollapsed.value) return;
  event.preventDefault();
  stopSftpTreeResize?.();
  const startX = event.clientX;
  const startWidth = sftpTreeWidth.value;
  const handleMove = (moveEvent: PointerEvent) => {
    sftpTreeWidth.value = Math.min(420, Math.max(160, startWidth + moveEvent.clientX - startX));
  };
  const handleStop = () => {
    window.removeEventListener("pointermove", handleMove);
    window.removeEventListener("pointerup", handleStop);
    localStorage.setItem("liteshell.sftp.tree-width.v1", String(sftpTreeWidth.value));
    stopSftpTreeResize = null;
  };
  stopSftpTreeResize = handleStop;
  window.addEventListener("pointermove", handleMove);
  window.addEventListener("pointerup", handleStop, { once: true });
}

async function loadDirectory(sessionId: string, path: string, recordHistory = true) {
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
    void synchronizeDirectoryTreePath(sessionId, listing.path);
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
}

function navigateHistory(offset: number) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const next = state.historyIndex + offset;
  if (next < 0 || next >= state.history.length) return;
  state.historyIndex = next;
  void loadDirectory(sessionId, state.history[next], false);
}

function parentPath(path: string) {
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
}

function joinRemotePath(directory: string, name: string) {
  return `${directory.replace(/\/$/, "")}/${name}`;
}

function joinLocalPath(directory: string, name: string) {
  return `${directory.replace(/[\\/]+$/, "")}\\${name.replaceAll("/", "\\")}`;
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

type FileQueueRequest = {
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
): Promise<boolean> {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return false;
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
      return false;
    }
    let directoryStrategy: DirectoryConflictStrategy = "merge";
    if (rootInspection.kind === "directory") {
      const choice = await chooseDirectoryConflict(manifest.rootName, conflicts, allowDirectoryAll);
      if (choice === "cancel") return false;
      if (choice === "skip") return true;
      directoryStrategy = choice;
    }
    prepared = await prepareRemoteDirectory(
      sessionId,
      requestedRoot,
      directoryStrategy,
      directoryStrategy === "replace" ? crypto.randomUUID() : undefined,
    );
    if (prepared.skipped) return true;

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
          return false;
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
    return true;
  } catch (error) {
    if (prepared?.replacementId) {
      try {
        await finishPreparedDirectory(prepared, false, sessionId);
      } catch (rollbackError) {
        state.error = `${describeCommandError(error)}；自动恢复原目录失败：${describeCommandError(rollbackError)}`;
        return false;
      }
    }
    state.error = describeCommandError(error);
    return false;
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
  if (item.state === "queued") {
    const connected = Boolean(item.availableSessionId)
      || sessions.value.some((session) => session.connected && session.id === item.sessionId);
    return connected ? "排队中" : "等待连接";
  }
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

async function createRemoteDirectory() {
  const sessionId = activeSessionId.value;
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return;
  const state = ensureSftpSessionState(sftpStates, sessionId);
  const name = window.prompt("新建目录名称");
  if (!name?.trim() || /[\\/]/.test(name)) {
    if (name) state.error = "目录名称不能包含路径分隔符";
    return;
  }
  if (activeSessionId.value !== sessionId) return;
  try {
    await createSftpDirectory(sessionId, joinRemotePath(state.path, name.trim()));
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function renameRemoteEntry() {
  const selected = selectedRemoteFile.value;
  if (!selected) return;
  const sessionId = selected.sessionId;
  const session = sessions.value.find((item) => item.id === sessionId);
  const state = sftpStates.get(sessionId);
  if (!session?.connected || !state || activeSessionId.value !== sessionId) return;
  const name = window.prompt("新的名称", selected.name);
  if (!name?.trim() || name === selected.name || /[\\/]/.test(name)) return;
  if (activeSessionId.value !== sessionId || !state.selectedEntries.some((entry) => entry.path === selected.path)) {
    state.error = "会话或选择已经变化，请重新选择后操作";
    return;
  }
  try {
    await renameSftpEntry(sessionId, selected.path, joinRemotePath(state.path, name.trim()));
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
}

async function deleteRemoteEntries() {
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

  const currentPaths = state.selectedEntries.map((entry) => entry.path);
  const selectedPaths = selected.map((entry) => entry.path);
  if (activeSessionId.value !== sessionId || !selectionsMatchSnapshot(currentPaths, selectedPaths)) {
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

function formatSize(size: number, kind: SftpEntry["kind"]) {
  if (kind === "directory") return "-";
  if (size < 1024) return `${size} B`;
  if (size < 1024 ** 2) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 ** 3) return `${(size / 1024 ** 2).toFixed(1)} MB`;
  return `${(size / 1024 ** 3).toFixed(1)} GB`;
}

function formatModified(timestamp?: number) {
  return timestamp ? new Date(timestamp * 1000).toLocaleString("zh-CN", { hour12: false }) : "-";
}

async function windowAction(action: "minimize" | "maximize" | "close") {
  if (!isTauri()) return;
  const appWindow = getCurrentWindow();
  if (action === "minimize") await appWindow.minimize();
  if (action === "maximize") await appWindow.toggleMaximize();
  if (action === "close") await appWindow.close();
}

async function refreshMetrics() {
  const session = activeSession.value;
  if (!session?.connected || !isTauri() || monitorBusy.value) return;
  monitorBusy.value = true;
  refreshedAt.value = "更新中";
  try {
    const next = await fetchSystemMetrics(session.id);
    if (session.id !== activeSessionId.value) return;
    systemMetrics.value = next;
    monitorError.value = "";
    networkRxHistory.value = [...networkRxHistory.value.slice(-31), next.networkRxBytesPerSecond];
    networkTxHistory.value = [...networkTxHistory.value.slice(-31), next.networkTxBytesPerSecond];
    refreshedAt.value = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    await nextTick();
    drawChart();
  } catch (error) {
    monitorError.value = describeCommandError(error);
    refreshedAt.value = "不可用";
  } finally {
    monitorBusy.value = false;
  }
}

function drawChart() {
  const canvas = chartCanvas.value;
  if (!canvas) return;
  const box = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.round(box.width * dpr));
  canvas.height = Math.max(1, Math.round(box.height * dpr));
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, box.width, box.height);
  ctx.strokeStyle = "rgba(121, 148, 164, .2)";
  ctx.lineWidth = 1;
  for (let y = 12; y < box.height; y += 28) {
    ctx.beginPath(); ctx.moveTo(0, y + .5); ctx.lineTo(box.width, y + .5); ctx.stroke();
  }
  const maxValue = Math.max(1024, ...networkRxHistory.value, ...networkTxHistory.value);
  const draw = (values: number[], color: string) => {
    ctx.strokeStyle = color; ctx.lineWidth = 1.7; ctx.beginPath();
    values.forEach((value, index) => {
      const x = (index / (values.length - 1)) * box.width;
      const y = box.height - 8 - (value / maxValue) * (box.height - 18);
      index === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    });
    ctx.stroke();
  };
  draw(networkRxHistory.value, "#5db6f7");
  draw(networkTxHistory.value, "#77d35d");
}

function percentage(used?: number, total?: number) {
  return total ? Math.min(100, Math.max(0, used! / total * 100)) : 0;
}

function formatBytes(value = 0) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MB`;
  if (value < 1024 ** 4) return `${(value / 1024 ** 3).toFixed(1)} GB`;
  return `${(value / 1024 ** 4).toFixed(1)} TB`;
}

function formatRate(value = 0) {
  return `${formatBytes(value)}/s`;
}

function formatEta(seconds?: number | null) {
  if (seconds === undefined || seconds === null) return "--";
  if (seconds < 60) return `${seconds} 秒`;
  if (seconds < 3600) return `${Math.ceil(seconds / 60)} 分钟`;
  return `${Math.floor(seconds / 3600)} 小时 ${Math.ceil(seconds % 3600 / 60)} 分钟`;
}

function isSftpDropPosition(position: { x: number; y: number }) {
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
        directoryCount += manifest.directoryCount + 1;
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
      const shouldContinue = await uploadDirectoryPath(
        directory.path,
        directory.manifest,
        preview.sessionId,
        conflicts,
        preview.directories.length > 1,
      );
      if (!shouldContinue || state.error) return;
    }
    if (preview.files.length) await uploadFilePaths(preview.files, preview.sessionId, conflicts);
  } finally {
    sftpDropBusy.value = false;
    sftpDropPreview.value = null;
  }
}

function formatUptime(seconds = 0) {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor(seconds % 86400 / 3600);
  const minutes = Math.floor(seconds % 3600 / 60);
  return `${days} 天 ${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

onMounted(async () => {
  await nextTick();
  drawChart();
  window.addEventListener("resize", drawChart, { passive: true });
  monitorTimer = window.setInterval(() => void refreshMetrics(), 3000);

  if (terminalContainer.value) {
    terminal = new Terminal({
      cursorBlink: true,
      fontFamily: '"Cascadia Mono", "JetBrains Mono", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.2,
      scrollback: 10_000,
      convertEol: false,
      theme: {
        background: "#07151d",
        foreground: "#dce4e8",
        cursor: "#dbe3e7",
        green: "#78dc53",
        brightGreen: "#8bea62",
        red: "#ef6b73",
        blue: "#58b7fb",
      },
    });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(terminalContainer.value);
    renderActiveTerminal();
    terminal.onData((data) => {
      const session = activeSession.value;
      if (session?.connected && isTauri()) void sendSshInput(session.id, data);
    });
    terminal.onResize(({ cols, rows }) => {
      const session = activeSession.value;
      if (session?.connected && isTauri()) void resizeSsh(session.id, cols, rows);
    });
    terminalResizeObserver = new ResizeObserver(() => fitAddon?.fit());
    terminalResizeObserver.observe(terminalContainer.value);
  }

  if (isTauri()) {
    unlistenSsh = await listenSshEvents(handleSshEvent);
    unlistenSftp = await listenSftpQueueTasks(handleTransfer);
    unlistenDragDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        sftpDragActive.value = Boolean(activeSession.value?.connected)
          && isSftpDropPosition(event.payload.position);
      } else if (event.payload.type === "drop") {
        const insideSftp = isSftpDropPosition(event.payload.position);
        sftpDragActive.value = false;
        if (insideSftp) void prepareDroppedPaths(event.payload.paths);
      } else sftpDragActive.value = false;
    });
    profiles.value = await listProfiles().catch(() => []);
    await refreshTransferQueue();
  }
});

watch(fileDoubleClickAction, (value) => {
  localStorage.setItem("liteshell.sftp.file-double-click.v1", value);
});

watch(activeSessionId, (sessionId) => {
  sftpContextMenu.value = null;
  sftpPathEditing.value = false;
  sftpPathDraft.value = sessionId ? ensureSftpSessionState(sftpStates, sessionId).path : "";
  renderActiveTerminal();
  if (sessionId) loadSftpStorage(sessionId);
  const session = sessions.value.find((item) => item.id === sessionId);
  if (session?.connected) {
    const state = ensureSftpSessionState(sftpStates, sessionId);
    void loadDirectory(sessionId, state.path, false);
    void refreshMetrics();
  } else {
    systemMetrics.value = null;
    networkRxHistory.value = Array(32).fill(0);
    networkTxHistory.value = Array(32).fill(0);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", drawChart);
  terminalResizeObserver?.disconnect();
  terminal?.dispose();
  unlistenSsh?.();
  unlistenSftp?.();
  unlistenDragDrop?.();
  stopSftpTreeResize?.();
  if (monitorTimer !== null) window.clearInterval(monitorTimer);
});
</script>

<template>
  <div class="app-shell" :class="{ 'sidebar-collapsed': sidebarCollapsed }" @click="sftpContextMenu = null">
    <header class="titlebar" data-tauri-drag-region>
      <div class="brand-mark"><IconServer :size="16" stroke-width="2.1" /></div>
      <strong>LiteShell</strong>
      <button class="window-menu icon-button" aria-label="主菜单"><IconMenu2 :size="18" /></button>
      <div class="window-controls" aria-label="窗口控制"><button aria-label="最小化" @click="windowAction('minimize')">—</button><button aria-label="最大化" @click="windowAction('maximize')">□</button><button aria-label="关闭" @click="windowAction('close')">×</button></div>
    </header>

    <aside class="sidebar">
      <div class="sidebar-heading"><span>连接管理</span><button @click="showConnectionManager = true">管理器</button></div>
      <div class="host-picker"><span class="online-dot"></span><strong>{{ activeSession?.name ?? '未选择连接' }}</strong><IconChevronDown :size="15" /><button class="icon-button" aria-label="新建连接" @click="addSession"><IconSquarePlus :size="20" /></button></div>
      <label class="search-field"><IconSearch :size="17" /><input v-model="search" aria-label="搜索主机" placeholder="搜索主机 (Ctrl+Shift+F)" /><IconAdjustmentsHorizontal :size="18" /></label>
      <div class="saved-connections">
        <div class="saved-connections-heading"><span>已保存连接</span><small>{{ profiles.length }}</small></div>
        <div v-for="profile in visibleProfiles" :key="profile.id" class="saved-connection-row">
          <button class="saved-connection-main" @click="openProfile(profile)"><span class="online-dot offline"></span><span><strong>{{ profile.name }}</strong><small>{{ profile.username }}@{{ profile.host }}:{{ profile.port }}</small></span><IconStarFilled v-if="profile.favorite" :size="14" /></button>
          <button class="icon-button delete-profile" :aria-label="`删除 ${profile.name}`" @click="removeProfile(profile)"><IconTrash :size="15" /></button>
        </div>
        <span v-if="!visibleProfiles.length" class="empty-connections">{{ search ? '没有匹配的连接' : '暂无保存的连接' }}</span>
      </div>

      <section class="monitor-panel">
        <div class="panel-heading">
          <span>系统状态</span><small>{{ refreshedAt }}</small>
          <button class="icon-button" aria-label="刷新系统状态" @click="refreshMetrics"><IconRefresh :size="16" /></button>
          <button class="icon-button" :aria-label="statusExpanded ? '折叠系统状态' : '展开系统状态'" @click="statusExpanded = !statusExpanded"><component :is="statusExpanded ? IconChevronUp : IconChevronDown" :size="16" /></button>
        </div>
        <div v-show="statusExpanded" class="metrics">
          <div class="uptime"><span>运行时间</span><strong>{{ formatUptime(systemMetrics?.uptimeSeconds) }}</strong></div>
          <div class="metric"><div><span>CPU {{ cpuPercent.toFixed(1) }}%</span><span>{{ systemMetrics?.cpuCores ?? 0 }} 核 · 负载 {{ systemMetrics?.loadAverage[0]?.toFixed(2) ?? '--' }}</span></div><div class="segments green"><i v-for="i in 28" :key="i" :class="{ on: i <= cpuPercent / 100 * 28 }"></i></div></div>
          <div class="metric"><div><span>内存 {{ memoryPercent.toFixed(1) }}%</span><span>{{ formatBytes(systemMetrics?.memoryUsed) }} / {{ formatBytes(systemMetrics?.memoryTotal) }}</span></div><div class="segments orange"><i v-for="i in 28" :key="i" :class="{ on: i <= memoryPercent / 100 * 28 }"></i></div></div>
          <div class="metric"><div><span>交换 {{ swapPercent.toFixed(1) }}%</span><span>{{ formatBytes(systemMetrics?.swapUsed) }} / {{ formatBytes(systemMetrics?.swapTotal) }}</span></div><div class="segments"><i v-for="i in 28" :key="i" :class="{ on: i <= swapPercent / 100 * 28 }"></i></div></div>
          <div v-if="monitorError" class="monitor-error">{{ monitorError }}</div>
        </div>
      </section>

      <section class="monitor-panel network-panel">
        <div class="panel-heading"><span>网络</span><strong>{{ systemMetrics ? `${systemMetrics.latencyMs} ms` : '--' }}</strong></div>
        <div class="chart-scale"><span>{{ formatRate(networkScale) }}</span><span>{{ formatRate(networkScale / 2) }}</span><span>0 B/s</span></div>
        <canvas ref="chartCanvas" class="network-chart" aria-label="实时网络流量图"></canvas>
        <div class="chart-time"><span>1m</span><span>40s</span><span>20s</span><span>现在</span></div>
        <div class="chart-legend"><span class="up">上行: {{ formatRate(systemMetrics?.networkTxBytesPerSecond) }}</span><span class="down">下行: {{ formatRate(systemMetrics?.networkRxBytesPerSecond) }}</span></div>
      </section>

      <section class="monitor-panel disk-panel">
        <div class="panel-heading"><span>磁盘</span></div>
        <div class="disk-header"><span>路径</span><span>剩余/总计</span><span>使用率</span></div>
        <div v-for="disk in systemMetrics?.disks ?? []" :key="disk.path" class="disk-row"><span>{{ disk.path }}</span><span>{{ formatBytes(Math.max(0, disk.total - disk.used)) }} / {{ formatBytes(disk.total) }}</span><span>{{ disk.usagePercent.toFixed(0) }}%</span><div><i :style="{ width: `${Math.min(100, disk.usagePercent)}%` }"></i></div></div>
        <div v-if="!systemMetrics?.disks.length" class="disk-empty">暂无数据</div>
      </section>
      <button class="collapse-sidebar" aria-label="折叠侧栏" @click="sidebarCollapsed = !sidebarCollapsed"><IconChevronLeft :size="17" /></button>
    </aside>

    <main class="workspace">
      <nav class="session-tabs" aria-label="会话标签">
        <button v-for="session in sessions" :key="session.id" class="session-tab" :class="{ active: session.id === activeSessionId }" @click="activeSessionId = session.id">
          <span v-if="session.connected" class="online-dot"></span><span>{{ session.name }}</span><IconX :size="15" @click.stop="closeSession(session.id)" />
        </button>
        <button class="add-tab icon-button" aria-label="添加会话" @click="addSession"><IconPlus :size="20" /></button>
      </nav>

      <section class="terminal-pane" aria-label="SSH 终端">
        <div ref="terminalContainer" class="terminal-host" :class="{ 'terminal-host-hidden': !activeSession }"></div>
        <div v-if="!activeSession" class="quick-connect">
          <div class="quick-connect-title"><div><IconServer :size="25" /></div><span><strong>快速连接</strong><small>输入服务器信息，立即打开 SSH 会话</small></span></div>
          <form @submit.prevent="submitConnection()">
            <label class="quick-host"><span>主机地址</span><input v-model="connectionForm.host" autocomplete="off" placeholder="IP 地址或域名" autofocus /></label>
            <label><span>端口</span><input v-model.number="connectionForm.port" type="number" min="1" max="65535" /></label>
            <label><span>用户名</span><input v-model="connectionForm.username" autocomplete="username" /></label>
            <label class="quick-password"><span>密码</span><input v-model="connectionForm.password" type="password" autocomplete="current-password" placeholder="SSH 登录密码" /></label>
            <div class="quick-connect-options"><label><input v-model="connectionForm.rememberProfile" type="checkbox" />保存连接</label><label><input v-model="connectionForm.rememberSecret" type="checkbox" :disabled="!connectionForm.rememberProfile" />保存密码</label></div>
            <p v-if="connectError" class="quick-connect-error">{{ connectError }}</p>
            <button class="quick-connect-button" type="submit" :disabled="connectBusy">{{ connectBusy ? '连接中…' : '立即连接' }}</button>
          </form>
          <div v-if="profiles.length" class="quick-saved"><header><span>已保存连接</span><button @click="showConnectionManager = true">管理全部</button></header><div><button v-for="profile in profiles.slice(0, 6)" :key="profile.id" @click="connectManagedProfiles([profile])"><span class="online-dot offline"></span><span><strong>{{ profile.name }}</strong><small>{{ profile.username }}@{{ profile.host }}</small></span></button></div></div>
          <button class="quick-more" @click="addSession">私钥、分组及更多连接选项</button>
        </div>
      </section>

      <section
        ref="sftpPaneElement"
        class="sftp-pane"
        :style="{ '--sftp-tree-width': `${sftpTreeCollapsed ? 0 : sftpTreeWidth}px` }"
      >
        <div v-if="sftpDragActive" class="sftp-drop-overlay"><IconUpload :size="38" /><strong>上传到 {{ sftpPath }}</strong><span>松开鼠标上传文件或文件夹</span></div>
        <aside class="sftp-tree-pane" :class="{ collapsed: sftpTreeCollapsed }">
          <div class="sftp-tree-tabs">
            <button :class="{ active: selectedTool === 'files' }" @click="selectedTool = 'files'"><IconFolder :size="15" />目录</button>
            <button :class="{ active: selectedTool === 'bookmarks' }" @click="selectedTool = 'bookmarks'"><IconStar :size="15" />书签</button>
            <button :class="{ active: selectedTool === 'history' }" @click="selectedTool = 'history'"><IconClockHour4 :size="15" />历史</button>
            <button class="sftp-tree-collapse icon-button" aria-label="折叠目录树" @click="sftpTreeCollapsed = true"><IconChevronLeft :size="16" /></button>
          </div>
          <SftpDirectoryTree
            v-if="selectedTool === 'files'"
            :state="activeDirectoryTreeState"
            :connected="Boolean(activeSession?.connected)"
            @open="openDirectoryTreePath"
            @toggle="toggleDirectoryTreeNode"
            @refresh="refreshDirectoryTreeNode"
          />
          <div v-else-if="selectedTool === 'bookmarks'" class="sftp-tree-location-list">
            <header><strong>路径书签</strong><span>{{ sftpBookmarks.length }} 项</span></header>
            <button v-for="path in sftpBookmarks" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconBookmark :size="15" /><span>{{ path }}</span><IconX :size="14" @click.stop="removeSftpBookmark(path)" /></button>
            <div v-if="!sftpBookmarks.length" class="sftp-directory-tree-empty"><IconBookmark :size="28" /><strong>暂无书签</strong><span>在地址栏收藏常用目录</span></div>
          </div>
          <div v-else class="sftp-tree-location-list">
            <header><strong>访问历史</strong><button :disabled="!sftpRecentPaths.length" @click="clearSftpHistory">清空</button></header>
            <button v-for="path in sftpRecentPaths" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconClockHour4 :size="15" /><span>{{ path }}</span></button>
            <div v-if="!sftpRecentPaths.length" class="sftp-directory-tree-empty"><IconClockHour4 :size="28" /><strong>暂无历史</strong><span>浏览过的远程目录会显示在这里</span></div>
          </div>
        </aside>
        <div class="sftp-tree-resizer" role="separator" aria-orientation="vertical" @pointerdown="beginSftpTreeResize"></div>
        <button v-if="sftpTreeCollapsed" class="sftp-tree-expand icon-button" aria-label="展开目录树" @click="sftpTreeCollapsed = false"><IconChevronRight :size="17" /></button>
        <div class="file-browser">
          <div class="file-toolbar">
            <button class="icon-button" aria-label="后退" :disabled="sftpHistoryIndex <= 0" @click="navigateHistory(-1)"><IconArrowLeft :size="20" /></button><button class="icon-button" aria-label="前进" :disabled="sftpHistoryIndex >= sftpHistory.length - 1" @click="navigateHistory(1)"><IconArrowRight :size="20" /></button><button class="icon-button" aria-label="上一级" @click="loadActiveDirectory(parentPath(sftpPath))"><IconArrowUp :size="20" /></button><button class="icon-button" aria-label="刷新" :disabled="sftpLoading" @click="loadActiveDirectory(sftpPath, false)"><IconRefresh :size="19" /></button>
            <div class="path-field">
              <input v-if="sftpPathEditing" ref="sftpPathInput" v-model="sftpPathDraft" aria-label="编辑远程路径" @keyup.enter="submitPathEditing" @keyup.esc="cancelPathEditing" />
              <nav v-else class="path-breadcrumbs" aria-label="远程路径面包屑"><button v-for="crumb in sftpBreadcrumbs" :key="crumb.path" @click="loadActiveDirectory(crumb.path)">{{ crumb.label }}</button></nav>
              <button class="path-edit-button" :aria-label="sftpPathEditing ? '取消编辑路径' : '编辑远程路径'" @click="sftpPathEditing ? cancelPathEditing() : beginPathEditing()">{{ sftpPathEditing ? '取消' : '编辑' }}</button>
              <button class="icon-button" aria-label="收藏路径" @click="toggleSftpBookmark"><component :is="starred ? IconStarFilled : IconStar" :size="20" /></button>
            </div>
            <label class="sftp-search"><IconSearch :size="15" /><input v-model="sftpSearch" placeholder="筛选" /></label>
            <label class="sftp-option"><input v-model="showHiddenFiles" type="checkbox" />显示隐藏文件</label>
            <label class="sftp-option">双击文件<select v-model="fileDoubleClickAction"><option value="select">仅选择</option><option value="download">下载</option></select></label>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUpload"><IconUpload :size="18" />上传文件</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUploadDirectory"><IconFolder :size="17" />上传目录</button>
            <button class="toolbar-button" :disabled="!selectedRemoteFiles.length" @click="startDownload"><IconArrowDown :size="18" />下载{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="createRemoteDirectory"><IconPlus :size="18" />新建目录</button>
            <button class="toolbar-button" :disabled="selectedRemoteFiles.length !== 1" @click="renameRemoteEntry">重命名</button>
            <button class="toolbar-button danger" :disabled="!selectedRemoteFiles.length" @click="deleteRemoteEntries"><IconTrash :size="16" />删除{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>
          </div>
          <div v-if="recursiveScan && recursiveScan.sessionId === activeSessionId" class="sftp-scan-status"><span>{{ recursiveScan.label }}…</span><button @click="cancelRecursiveScan">取消扫描</button></div>
          <div v-if="visibleTransfers.length" class="transfer-queue">
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
          <div v-if="activeSftpState.notice" class="sftp-notice">{{ activeSftpState.notice }}</div>
          <div v-if="sftpError" class="sftp-error">{{ sftpError }}</div>
          <div class="file-table" role="table" aria-label="远程文件">
            <div class="file-row file-header" role="row"><button @click="changeSftpSort('name')">名称</button><button @click="changeSftpSort('size')">大小</button><button @click="changeSftpSort('modifiedAt')">修改时间</button><span>权限</span></div>
            <button v-for="file in displayedSftpEntries" :key="file.path" class="file-row" :class="{ selected: selectedRemoteFiles.some(item => item.path === file.path) }" role="row" @click="selectRemoteEntry(file, $event)" @dblclick="openSftpEntry(file)" @contextmenu.prevent="openSftpContextMenu(file, $event)"><span><component :is="file.kind === 'directory' ? IconFolder : IconFile" :size="21" :class="file.kind === 'directory' ? 'folder' : 'file'" />{{ file.name }}</span><span>{{ formatSize(file.size, file.kind) }}</span><span>{{ formatModified(file.modifiedAt) }}</span><span>{{ file.permissions }}</span></button>
            <div v-if="sftpLoading" class="sftp-loading">正在读取目录…</div>
            <div v-else-if="activeSession?.connected && !displayedSftpEntries.length && !sftpError" class="sftp-loading">{{ sftpSearch ? '没有匹配的文件' : '目录为空' }}</div>
          </div>
          <footer class="file-summary">{{ sftpEntries.length }} 项 · {{ formatSize(currentDirectoryTotalSize, 'file') }}<span v-if="selectedRemoteFiles.length"> · 已选择 {{ selectedRemoteFiles.length }} 项</span></footer>
        </div>
      </section>
    </main>

    <footer class="statusbar"><span><i class="online-dot" :class="{ offline: !activeSession?.connected }"></i>{{ activeSession?.connected ? '已连接' : '未连接' }}</span><span>SFTP {{ sftpEntries.length }} 项 / 已选 {{ selectedRemoteFiles.length }} / {{ formatSize(currentDirectoryTotalSize, 'file') }}</span><span>运行传输 {{ runningTransferCount }}</span><span class="latency">{{ systemMetrics ? `${systemMetrics.latencyMs} ms` : '--' }}</span><span>UTF-8</span></footer>

    <div v-if="showConnectDialog" class="dialog-backdrop" @click.self="showConnectDialog = false">
      <section class="connect-dialog" role="dialog" aria-modal="true" aria-labelledby="connect-title">
        <header><div><IconServer :size="21" /><strong id="connect-title">新建 SSH 连接</strong></div><button class="icon-button" aria-label="关闭连接窗口" @click="showConnectDialog = false"><IconX :size="19" /></button></header>
        <template v-if="!pendingFingerprint">
          <div class="connection-grid">
            <label class="wide"><span>连接名称</span><input v-model="connectionForm.name" placeholder="例如：生产 API" /></label>
            <label class="wide"><span>主机地址</span><input v-model="connectionForm.host" autocomplete="off" placeholder="example.com 或 192.168.1.10" /></label>
            <label><span>端口</span><input v-model.number="connectionForm.port" type="number" min="1" max="65535" /></label>
            <label><span>用户名</span><input v-model="connectionForm.username" autocomplete="username" /></label>
            <label><span>分组</span><input v-model="connectionForm.group" placeholder="默认分组" /></label>
            <label class="check-field"><input v-model="connectionForm.favorite" type="checkbox" /><span>收藏连接</span></label>
            <fieldset class="wide"><legend>认证方式</legend><label><input v-model="connectionForm.authType" type="radio" value="password" />密码</label><label><input v-model="connectionForm.authType" type="radio" value="private_key" />私钥</label></fieldset>
            <label v-if="connectionForm.authType === 'password'" class="wide"><span>密码</span><input v-model="connectionForm.password" type="password" autocomplete="current-password" :placeholder="connectionForm.profileId ? '留空则使用已保存密码' : ''" /></label>
            <template v-else><label class="wide"><span>私钥路径</span><input v-model="connectionForm.privateKeyPath" placeholder="C:\\Users\\name\\.ssh\\id_ed25519" /></label><label class="wide"><span>私钥口令（可选）</span><input v-model="connectionForm.passphrase" type="password" :placeholder="connectionForm.profileId ? '留空则使用已保存口令' : ''" /></label></template>
            <div class="wide remember-options"><label><input v-model="connectionForm.rememberProfile" type="checkbox" />保存连接配置</label><label :class="{ disabled: !connectionForm.rememberProfile }"><input v-model="connectionForm.rememberSecret" type="checkbox" :disabled="!connectionForm.rememberProfile" />使用 Windows 凭据管理器保存密码/口令</label></div>
          </div>
        </template>
        <div v-else class="fingerprint-confirmation">
          <IconShieldCheck :size="38" />
          <strong>确认服务器主机密钥</strong>
          <p>这是首次连接到 <b>{{ connectionForm.host }}:{{ connectionForm.port }}</b>。请在可信渠道核对指纹后再继续。</p>
          <dl><div><dt>算法</dt><dd>{{ pendingFingerprint.algorithm }}</dd></div><div><dt>SHA-256 指纹</dt><dd>{{ pendingFingerprint.fingerprint }}</dd></div></dl>
        </div>
        <p v-if="connectError" class="dialog-error">{{ connectError }}</p>
        <footer><button class="secondary-button" @click="showConnectDialog = false">取消</button><button v-if="pendingFingerprint" class="primary-button" :disabled="connectBusy" @click="confirmHostKey">信任并连接</button><button v-else class="primary-button" :disabled="connectBusy" @click="submitConnection()">{{ connectBusy ? '连接中…' : '连接' }}</button></footer>
      </section>
    </div>
    <div v-if="sftpContextMenu" class="sftp-context-menu" :style="{ left: `${sftpContextMenu.x}px`, top: `${sftpContextMenu.y}px` }" @click.stop>
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
    <ConnectionManager :open="showConnectionManager" @close="showConnectionManager = false" @changed="profiles = $event.profiles" @connect="connectManagedProfiles" />
    <div v-if="conflictRequest" class="dialog-backdrop conflict-backdrop">
      <section class="conflict-dialog" role="dialog" aria-modal="true" :aria-label="conflictRequest.kind === 'directory' ? '同名目录处理' : '同名文件处理'">
        <header><strong>{{ conflictRequest.kind === 'directory' ? '目标中已存在同名目录' : '目标中已存在同名文件' }}</strong></header>
        <p>“{{ conflictRequest.name }}”已存在，请选择处理方式。</p>
        <p v-if="conflictRequest.kind === 'directory'" class="conflict-explanation">合并会保留目标中的额外内容；替换会先写入独立临时目录，提交后删除目标中源目录不存在的额外内容。</p>
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
  </div>
</template>
