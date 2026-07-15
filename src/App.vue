<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import "@xterm/xterm/css/xterm.css";
import ConnectionManager from "./components/ConnectionManager.vue";
import {
  cancelSftpTransfer,
  commandErrorCode,
  connectProfile,
  connectSsh,
  createSftpDirectory,
  deleteSftpEntry,
  deleteSftpDirectoryRecursive,
  deleteProfile,
  deleteSftpTransferCheckpoint,
  discardSftpTransferCheckpoint,
  describeCommandError,
  downloadSftpFile,
  fetchSystemMetrics,
  finishDirectoryReplacement,
  getLocalDirectoryManifest,
  getRemoteDirectoryManifest,
  disconnectSsh,
  inspectLocalPath,
  inspectRemotePath,
  isTauri,
  listProfiles,
  listSftpDirectory,
  listSftpTransferCheckpoints,
  prepareLocalDirectory,
  prepareRemoteDirectory,
  listenSftpTransfers,
  listenSshEvents,
  resizeSsh,
  renameSftpEntry,
  saveProfile,
  sendSshInput,
  uploadSftpFile,
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
  type TransferCheckpoint,
  type TransferEvent,
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
import {
  IconAdjustmentsHorizontal,
  IconArrowDown,
  IconArrowLeft,
  IconArrowRight,
  IconArrowUp,
  IconBookmark,
  IconChevronDown,
  IconChevronLeft,
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
const transfers = ref<TransferEvent[]>([]);
const pendingTransferCheckpoints = ref<TransferCheckpoint[]>([]);
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
let monitorTimer: number | null = null;
const terminalBuffers = new Map<string, string>();
const textDecoder = new TextDecoder();
type TransferTask =
  | { taskId: string; direction: "upload"; sessionId: string; localPath: string; remotePath: string; conflictStrategy: ConflictStrategy; resume: boolean }
  | { taskId: string; direction: "download"; sessionId: string; remotePath: string; localPath: string; conflictStrategy: ConflictStrategy; resume: boolean };
const transferTasks = new Map<string, TransferTask>();

const visibleProfiles = computed(() => {
  const term = search.value.trim().toLowerCase();
  return profiles.value.filter((profile) =>
    !term || [profile.name, profile.host, profile.username, profile.group].some((value) => value.toLowerCase().includes(term)),
  );
});
const visibleTransfers = computed(() => transfers.value.filter((item) => item.sessionId === activeSessionId.value));
const cpuPercent = computed(() => systemMetrics.value?.cpuUsagePercent ?? 0);
const memoryPercent = computed(() => percentage(systemMetrics.value?.memoryUsed, systemMetrics.value?.memoryTotal));
const swapPercent = computed(() => percentage(systemMetrics.value?.swapUsed, systemMetrics.value?.swapTotal));
const networkScale = computed(() => Math.max(1024, ...networkRxHistory.value, ...networkTxHistory.value));
const activeSession = computed(() => sessions.value.find((session) => session.id === activeSessionId.value));
const checkpointSession = (checkpoint: TransferCheckpoint) =>
  sessions.value.find((session) => session.connected && session.id === checkpoint.availableSessionId);
const selectedRemoteFile = computed(() => selectedRemoteFiles.value.length === 1 ? selectedRemoteFiles.value[0] : null);
const starred = computed(() => sftpBookmarks.value.includes(sftpPath.value));
const displayedSftpEntries = computed(() => {
  const term = sftpSearch.value.trim().toLocaleLowerCase();
  const entries = sftpEntries.value.filter((entry) => !term || entry.name.toLocaleLowerCase().includes(term));
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
  if (["connected", "disconnected", "exit", "error"].includes(event.kind)) void refreshTransferCheckpoints();
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

async function loadDirectory(sessionId: string, path: string, recordHistory = true) {
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
}

function joinRemotePath(directory: string, name: string) {
  return `${directory.replace(/\/$/, "")}/${name}`;
}

function joinLocalPath(directory: string, name: string) {
  return `${directory.replace(/[\\/]+$/, "")}\\${name.replaceAll("/", "\\")}`;
}

async function runWithConcurrency<T>(items: T[], worker: (item: T) => Promise<void>, limit = 3) {
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
  const tasks: Array<{ localPath: string; remotePath: string; conflictStrategy: ConflictStrategy }> = [];
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
    const tasks: Array<{
      localPath: string;
      remotePath: string;
      conflictStrategy: ConflictStrategy;
    }> = [];
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
      tasks.push({ localPath: file.absolutePath, remotePath, conflictStrategy });
    }
    await runWithConcurrency(tasks, async (task) => {
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
    for (const directory of manifest.directories) {
      await prepareLocalDirectory(joinLocalPath(prepared.path, directory), "merge");
    }
    const downloads: Array<{
      remotePath: string;
      localPath: string;
      conflictStrategy: ConflictStrategy;
    }> = [];
    for (const file of manifest.files) {
      const localPath = joinLocalPath(prepared.path, file.relativePath);
      const inspection = await inspectLocalPath(localPath);
      if (inspection.kind === "directory" || inspection.kind === "other") {
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
      downloads.push({ remotePath: file.remotePath, localPath, conflictStrategy });
    }
    await runWithConcurrency(downloads, (download) => downloadOne(
      sessionId,
      download.remotePath,
      download.localPath,
      download.conflictStrategy,
    ));
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

async function downloadOne(sessionId: string, remotePath: string, localPath: string, conflictStrategy: ConflictStrategy) {
  const transferId = crypto.randomUUID();
  const taskId = crypto.randomUUID();
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return;
  const request = { taskId, direction: "download" as const, sessionId, remotePath, localPath, conflictStrategy, resume: false };
  transferTasks.set(transferId, request);
  const result = await downloadSftpFile({ sessionId, remotePath, localPath, transferId, taskId, conflictStrategy, resume: false });
  if (result.skipped) transferTasks.delete(transferId);
}

function handleTransfer(event: TransferEvent) {
  const index = transfers.value.findIndex((item) => item.transferId === event.transferId);
  if (index >= 0) {
    const previous = transfers.value[index];
    if (event.state !== "running" && event.transferred < previous.transferred) {
      event.transferred = previous.transferred;
    }
    transfers.value[index] = event;
  }
  else transfers.value.push(event);
  if (event.state === "completed") transferTasks.delete(event.transferId);
  if (event.state !== "running") void refreshTransferCheckpoints();
  if (event.state === "failed") {
    ensureSftpSessionState(sftpStates, event.sessionId).error = event.message ?? "文件传输失败";
  }
}

async function refreshTransferCheckpoints() {
  if (!isTauri()) return;
  pendingTransferCheckpoints.value = await listSftpTransferCheckpoints().catch(() => []);
}

function checkpointFileName(checkpoint: TransferCheckpoint) {
  const path = checkpoint.direction === "upload" ? checkpoint.sourcePath : checkpoint.targetPath;
  return path.split(/[\\/]/).pop() ?? path;
}

async function runRecoveredTransfer(checkpoint: TransferCheckpoint, resume: boolean) {
  const session = checkpointSession(checkpoint);
  if (!session) {
    window.alert("请先重新连接该传输所属的服务器");
    return;
  }
  activeSessionId.value = session.id;
  const transferId = crypto.randomUUID();
  const common = {
    taskId: checkpoint.taskId,
    sessionId: session.id,
    conflictStrategy: "overwrite" as ConflictStrategy,
    resume,
  };
  try {
    if (checkpoint.direction === "upload") {
      const task: TransferTask = {
        ...common,
        direction: "upload",
        localPath: checkpoint.sourcePath,
        remotePath: checkpoint.targetPath,
      };
      transferTasks.set(transferId, task);
      await uploadSftpFile({ ...task, transferId });
    } else {
      const task: TransferTask = {
        ...common,
        direction: "download",
        remotePath: checkpoint.sourcePath,
        localPath: checkpoint.targetPath,
      };
      transferTasks.set(transferId, task);
      await downloadSftpFile({ ...task, transferId });
    }
  } catch (error) {
    ensureSftpSessionState(sftpStates, session.id).error = describeCommandError(error);
  } finally {
    await refreshTransferCheckpoints();
  }
}

async function preserveCheckpointTemporaryFile(checkpoint: TransferCheckpoint) {
  await deleteSftpTransferCheckpoint(checkpoint.taskId);
  await refreshTransferCheckpoints();
}

async function discardCheckpointTemporaryFile(checkpoint: TransferCheckpoint) {
  const session = checkpoint.direction === "upload" ? checkpointSession(checkpoint) : undefined;
  if (checkpoint.direction === "upload" && !session) {
    window.alert("删除远程临时文件前请先重新连接对应服务器");
    return;
  }
  try {
    await discardSftpTransferCheckpoint(checkpoint.taskId, session?.id);
    await refreshTransferCheckpoints();
  } catch (error) {
    window.alert(describeCommandError(error));
  }
}

function transferProgress(item: TransferEvent) {
  return item.total > 0 ? Math.round((item.transferred / item.total) * 100) : 0;
}

function clearFinishedTransfers() {
  for (const item of transfers.value.filter((entry) => entry.state !== "running")) {
    transferTasks.delete(item.transferId);
  }
  transfers.value = transfers.value.filter((item) => item.state === "running");
}

async function cancelTransfer(item: TransferEvent) {
  if (item.state !== "running") return;
  await cancelSftpTransfer(item.transferId).catch((error) => {
    ensureSftpSessionState(sftpStates, item.sessionId).error = describeCommandError(error);
  });
}

async function retryTransfer(item: TransferEvent) {
  const task = transferTasks.get(item.transferId);
  if (!task || item.state === "running") return;
  const transferId = crypto.randomUUID();
  transferTasks.set(transferId, task);
  const state = ensureSftpSessionState(sftpStates, task.sessionId);
  try {
    if (task.direction === "upload") {
      await uploadSftpFile({ ...task, transferId, resume: true });
      await loadDirectory(task.sessionId, state.path, false);
    } else {
      await downloadSftpFile({ ...task, transferId, resume: true });
    }
  } catch (error) {
    state.error = describeCommandError(error);
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

async function deleteRemoteEntry() {
  const selected = selectedRemoteFile.value;
  if (!selected) return;
  const sessionId = selected.sessionId;
  const session = sessions.value.find((item) => item.id === sessionId);
  const state = sftpStates.get(sessionId);
  if (!session?.connected || !state || activeSessionId.value !== sessionId) return;
  const confirmed = await ask(`确定删除“${selected.name}”吗？此操作无法撤销。`, {
    title: selected.kind === "directory" ? "删除远程目录" : "删除远程文件",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });
  if (!confirmed) return;
  if (selected.kind === "directory") {
    const recursiveConfirmed = await ask(`目录“${selected.name}”中的所有文件和子目录都将被永久删除。请再次确认。`, {
      title: "确认递归删除",
      kind: "warning",
      okLabel: "永久删除目录",
      cancelLabel: "取消",
    });
    if (!recursiveConfirmed) return;
  }
  if (activeSessionId.value !== sessionId || !state.selectedEntries.some((entry) => entry.path === selected.path)) {
    state.error = "会话或选择已经变化，删除已取消";
    return;
  }
  try {
    if (selected.kind === "directory") {
      await deleteSftpDirectoryRecursive(sessionId, selected.path);
    } else {
      await deleteSftpEntry(sessionId, selected.path, false);
    }
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {
    state.error = describeCommandError(error);
  }
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

async function handleDroppedPaths(paths: string[]) {
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
    unlistenSftp = await listenSftpTransfers(handleTransfer);
    unlistenDragDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") sftpDragActive.value = Boolean(activeSession.value?.connected);
      else if (event.payload.type === "drop") {
        sftpDragActive.value = false;
        void handleDroppedPaths(event.payload.paths);
      } else sftpDragActive.value = false;
    });
    profiles.value = await listProfiles().catch(() => []);
    await refreshTransferCheckpoints();
  }
});

watch(activeSessionId, (sessionId) => {
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
  if (monitorTimer !== null) window.clearInterval(monitorTimer);
});
</script>

<template>
  <div class="app-shell" :class="{ 'sidebar-collapsed': sidebarCollapsed }">
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

      <section class="sftp-pane">
        <div v-if="sftpDragActive" class="sftp-drop-overlay"><IconUpload :size="38" /><strong>上传到 {{ sftpPath }}</strong><span>松开鼠标上传文件或文件夹</span></div>
        <aside class="sftp-tools">
          <strong>SFTP</strong>
          <button :class="{ active: selectedTool === 'files' }" @click="selectedTool = 'files'"><IconFolder :size="24" /><span>文件</span></button>
          <button :class="{ active: selectedTool === 'bookmarks' }" @click="selectedTool = 'bookmarks'"><IconStar :size="25" /><span>书签</span></button>
          <button :class="{ active: selectedTool === 'history' }" @click="selectedTool = 'history'"><IconClockHour4 :size="25" /><span>历史</span></button>
        </aside>
        <div class="file-browser">
          <div class="file-toolbar">
            <button class="icon-button" aria-label="后退" :disabled="sftpHistoryIndex <= 0" @click="navigateHistory(-1)"><IconArrowLeft :size="20" /></button><button class="icon-button" aria-label="前进" :disabled="sftpHistoryIndex >= sftpHistory.length - 1" @click="navigateHistory(1)"><IconArrowRight :size="20" /></button><button class="icon-button" aria-label="上一级" @click="loadActiveDirectory(parentPath(sftpPath))"><IconArrowUp :size="20" /></button><button class="icon-button" aria-label="刷新" :disabled="sftpLoading" @click="loadActiveDirectory(sftpPath, false)"><IconRefresh :size="19" /></button>
            <div class="path-field"><input v-model="sftpPath" aria-label="远程路径" @keyup.enter="loadActiveDirectory(sftpPath)" /><button class="icon-button" aria-label="收藏路径" @click="toggleSftpBookmark"><component :is="starred ? IconStarFilled : IconStar" :size="20" /></button></div>
            <label class="sftp-search"><IconSearch :size="15" /><input v-model="sftpSearch" placeholder="筛选" /></label>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUpload"><IconUpload :size="18" />上传文件</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="startUploadDirectory"><IconFolder :size="17" />上传目录</button>
            <button class="toolbar-button" :disabled="!selectedRemoteFiles.length" @click="startDownload"><IconArrowDown :size="18" />下载{{ selectedRemoteFiles.length > 1 ? `（${selectedRemoteFiles.length}）` : '' }}</button>
            <button class="toolbar-button" :disabled="!activeSession?.connected" @click="createRemoteDirectory"><IconPlus :size="18" />新建目录</button>
            <button class="toolbar-button" :disabled="selectedRemoteFiles.length !== 1" @click="renameRemoteEntry">重命名</button>
            <button class="toolbar-button danger" :disabled="selectedRemoteFiles.length !== 1" @click="deleteRemoteEntry"><IconTrash :size="16" />删除</button>
          </div>
          <div v-if="recursiveScan && recursiveScan.sessionId === activeSessionId" class="sftp-scan-status"><span>{{ recursiveScan.label }}…</span><button @click="cancelRecursiveScan">取消扫描</button></div>
          <div v-if="pendingTransferCheckpoints.length" class="transfer-queue checkpoint-queue">
            <div class="transfer-queue-heading"><span>未完成传输（{{ pendingTransferCheckpoints.length }}）</span></div>
            <div v-for="checkpoint in pendingTransferCheckpoints" :key="checkpoint.taskId" class="upload-strip failed">
              <span>{{ checkpoint.direction === 'upload' ? '上传' : '下载' }} {{ checkpointFileName(checkpoint) }}<small>已保存 {{ formatSize(checkpoint.transferred, 'file') }} / {{ formatSize(checkpoint.sourceSize, 'file') }}</small></span>
              <div><i :style="{ width: `${checkpoint.sourceSize ? Math.min(100, checkpoint.transferred / checkpoint.sourceSize * 100) : 0}%` }"></i></div>
              <span class="transfer-rate">{{ checkpointSession(checkpoint) ? '服务器已连接' : '等待重新连接服务器' }}</span>
              <strong>可恢复</strong>
              <button :disabled="!checkpointSession(checkpoint)" @click="runRecoveredTransfer(checkpoint, true)">继续</button>
              <button :disabled="!checkpointSession(checkpoint)" @click="runRecoveredTransfer(checkpoint, false)">重新开始</button>
              <button @click="preserveCheckpointTemporaryFile(checkpoint)">保留临时文件</button>
              <button :disabled="checkpoint.direction === 'upload' && !checkpointSession(checkpoint)" @click="discardCheckpointTemporaryFile(checkpoint)">删除临时文件</button>
            </div>
          </div>
          <div v-if="visibleTransfers.length" class="transfer-queue">
            <div class="transfer-queue-heading"><span>传输队列（{{ visibleTransfers.length }}）</span><button @click="clearFinishedTransfers">清除已完成</button></div>
            <div v-for="item in visibleTransfers" :key="item.transferId" class="upload-strip" :class="item.state"><span>{{ item.direction === 'upload' ? '上传' : '下载' }} {{ item.fileName }}<small v-if="item.resumedFrom">已续传 {{ formatSize(item.resumedFrom, 'file') }}</small></span><div><i :style="{ width: `${transferProgress(item)}%` }"></i></div><span class="transfer-rate">{{ item.state === 'running' ? `${formatRate(item.speedBytesPerSecond)} · 剩余 ${formatEta(item.etaSeconds)}` : '' }}</span><strong>{{ item.state === 'completed' ? '完成' : item.state === 'failed' ? '失败' : item.state === 'cancelled' ? '已取消' : `${transferProgress(item)}%` }}</strong><button v-if="item.state === 'running'" @click="cancelTransfer(item)">取消</button><button v-else-if="item.state !== 'completed'" @click="retryTransfer(item)">重试</button></div>
          </div>
          <div v-if="activeSftpState.notice" class="sftp-notice">{{ activeSftpState.notice }}</div>
          <div v-if="sftpError" class="sftp-error">{{ sftpError }}</div>
          <div v-if="selectedTool === 'files'" class="file-table" role="table" aria-label="远程文件">
            <div class="file-row file-header" role="row"><button @click="changeSftpSort('name')">名称</button><button @click="changeSftpSort('size')">大小</button><button @click="changeSftpSort('modifiedAt')">修改时间</button><span>权限</span></div>
            <button v-for="file in displayedSftpEntries" :key="file.path" class="file-row" :class="{ selected: selectedRemoteFiles.some(item => item.path === file.path) }" role="row" @click="selectRemoteEntry(file, $event)" @dblclick="openSftpEntry(file)"><span><component :is="file.kind === 'directory' ? IconFolder : IconFile" :size="21" :class="file.kind === 'directory' ? 'folder' : 'file'" />{{ file.name }}</span><span>{{ formatSize(file.size, file.kind) }}</span><span>{{ formatModified(file.modifiedAt) }}</span><span>{{ file.permissions }}</span></button>
            <div v-if="sftpLoading" class="sftp-loading">正在读取目录…</div>
            <div v-else-if="activeSession?.connected && !displayedSftpEntries.length && !sftpError" class="sftp-loading">{{ sftpSearch ? '没有匹配的文件' : '目录为空' }}</div>
          </div>
          <div v-else-if="selectedTool === 'bookmarks'" class="sftp-location-list"><header><strong>路径书签</strong><span>{{ sftpBookmarks.length }} 项</span></header><button v-for="path in sftpBookmarks" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconBookmark :size="17" /><span>{{ path }}</span><small>双击打开</small><IconX :size="15" @click.stop="removeSftpBookmark(path)" /></button><div v-if="!sftpBookmarks.length" class="empty-tool-state"><IconBookmark :size="34" /><strong>暂无书签</strong><span>在路径栏点击星标收藏常用目录</span></div></div>
          <div v-else class="sftp-location-list"><header><strong>访问历史</strong><button :disabled="!sftpRecentPaths.length" @click="clearSftpHistory">清空</button></header><button v-for="path in sftpRecentPaths" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconClockHour4 :size="17" /><span>{{ path }}</span><small>双击打开</small></button><div v-if="!sftpRecentPaths.length" class="empty-tool-state"><IconClockHour4 :size="34" /><strong>暂无历史记录</strong><span>浏览过的远程目录会显示在这里</span></div></div>
          <footer class="file-summary">{{ sftpEntries.length }} 项<span v-if="selectedRemoteFiles.length"> · 已选择 {{ selectedRemoteFiles.length }} 项</span></footer>
        </div>
      </section>
    </main>

    <footer class="statusbar"><span><i class="online-dot" :class="{ offline: !activeSession?.connected }"></i>{{ activeSession?.connected ? '已连接' : '未连接' }}</span><span class="latency">{{ systemMetrics ? `${systemMetrics.latencyMs} ms` : '--' }}</span><span>UTF-8</span><span>转发 0</span></footer>

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
