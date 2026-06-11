<template>
  <section class="sftp-layout">
    <header class="toolbar">
      <div>
        <h2>SFTP 文件</h2>
        <p>自动跟随工作台当前主机，并复用当前 SSH 内存认证。</p>
      </div>
      <div class="status" :class="{ 'status--online': connectionId }">
        {{ statusText }}
      </div>
    </header>

    <div class="content-grid">
      <section class="browser-card">
        <div class="status-strip" :class="{ 'status-strip--online': connectionId }">
          {{ statusNotice }}
        </div>

        <div class="sftp-toolbar-row">
          <div class="sftp-path-main" :title="currentPath">{{ currentPath }}</div>
          <div class="sftp-icon-actions" aria-label="SFTP 操作">
            <button class="sftp-icon-button" :disabled="!canBrowse || currentPath === '/'" title="上级" type="button" aria-label="上级" @click="goParent">
              ↖
            </button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="上传" type="button" aria-label="上传" @click="uploadFile">
              ↑
            </button>
            <button class="sftp-icon-button" :disabled="!canDownload" title="下载" type="button" aria-label="下载" @click="downloadFile">
              ↓
            </button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="新建目录" type="button" aria-label="新建目录" @click="createDirectory">
              ⊞
            </button>
            <button class="sftp-icon-button" :disabled="!canRunSelectedAction" title="重命名" type="button" aria-label="重命名" @click="renameItem">
              ✎
            </button>
            <button class="sftp-icon-button sftp-icon-button--danger" :disabled="!canRunSelectedAction" title="删除" type="button" aria-label="删除" @click="deleteItem">
              ⌫
            </button>
            <button class="sftp-icon-button" :disabled="autoConnecting || loading || actionLoading" title="刷新" type="button" aria-label="刷新" @click="refresh">
              ⟳
            </button>
          </div>
          <span v-if="actionLoading" class="action-message">{{ actionStatusText }}</span>
          <span v-else-if="actionMessage" class="action-message">{{ actionMessage }}</span>
        </div>

        <div v-if="errorMessage" class="error-box">{{ errorMessage }}</div>

        <div class="table-wrap">
          <table class="file-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>类型</th>
                <th>大小</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="!connectionId">
                <td colspan="3" class="empty-cell">{{ statusNotice }}</td>
              </tr>
              <tr v-else-if="loading">
                <td colspan="3" class="empty-cell">目录加载中...</td>
              </tr>
              <tr v-else-if="files.length === 0">
                <td colspan="3" class="empty-cell">当前目录为空。</td>
              </tr>
              <template v-else>
                <tr
                  v-for="file in files"
                  :key="file.path"
                  :class="{ 'file-row--dir': file.isDir, 'file-row--selected': selectedItem?.path === file.path }"
                  @click="selectItem(file)"
                  @dblclick="openItem(file)"
                >
                  <td class="name-cell">
                    <span class="file-icon">{{ file.isDir ? '📁' : '📄' }}</span>
                    <span>{{ file.name }}</span>
                  </td>
                  <td>{{ file.isDir ? '文件夹' : '文件' }}</td>
                  <td>{{ file.isDir ? '-' : formatSize(file.size) }}</td>
                </tr>
              </template>
            </tbody>
          </table>
        </div>

        <section v-if="transferTasks.length" class="transfer-panel">
          <div class="transfer-panel__head">
            <strong>传输任务</strong>
            <span>{{ activeTransferCount ? `${activeTransferCount} 个进行中` : '全部完成' }}</span>
          </div>
          <div class="transfer-list">
            <div v-for="task in transferTasks" :key="task.id" class="transfer-task">
              <div class="transfer-task__meta">
                <span class="transfer-task__type">{{ task.type === 'upload' ? '上传' : '下载' }}</span>
                <strong>{{ task.name }}</strong>
                <span>{{ transferStatusText(task.status) }}</span>
              </div>
              <div class="transfer-progress">
                <i :class="`transfer-progress__bar transfer-progress__bar--${task.status}`" :style="{ width: progressWidth(task.percent) }"></i>
              </div>
              <div class="transfer-task__foot">
                <span>{{ formatSize(task.transferredBytes) }} / {{ task.totalBytes ? formatSize(task.totalBytes) : '-' }}</span>
                <span>{{ formatPercent(task.percent) }}</span>
              </div>
            </div>
          </div>
        </section>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { type Event, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ask, open, save } from '@tauri-apps/plugin-dialog';

import { type WorkspaceCredential, type WorkspaceHost, useWorkspaceStore } from '@/stores/workspace';

interface RemoteFileItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

interface SftpSessionState {
  hostId: string;
  connectionId: string;
  currentPath: string;
  files: RemoteFileItem[];
  loading: boolean;
  connecting: boolean;
  errorMessage: string;
}

interface SftpTransferProgressPayload {
  transferId: string;
  transferredBytes: number;
  totalBytes: number;
  percent: number;
  status: TransferStatus;
}

type TransferStatus = 'running' | 'success' | 'failed';
type TransferType = 'upload' | 'download';

interface TransferTask {
  id: string;
  type: TransferType;
  name: string;
  transferredBytes: number;
  totalBytes: number;
  percent: number;
  status: TransferStatus;
  createdAt: number;
}

const workspaceStore = useWorkspaceStore();

const sessionsByHostId = shallowRef<Record<string, SftpSessionState>>({});
const selectedItem = ref<RemoteFileItem | null>(null);
const transferTasks = ref<TransferTask[]>([]);
const actionLoading = ref(false);
const actionMessage = ref('');
const actionStatusText = ref('');
let disposed = false;
let unlistenTransferProgress: UnlistenFn | undefined;

const activeHost = computed(() => workspaceStore.activeHost);
const cachedCredential = computed(() =>
  activeHost.value ? workspaceStore.getCredential(activeHost.value.id) : undefined,
);
const canAutoConnect = computed(() =>
  Boolean(activeHost.value && cachedCredential.value?.password),
);
const currentSession = computed(() => {
  const hostId = activeHost.value?.id;
  return hostId ? sessionsByHostId.value[hostId] : undefined;
});
const connectionId = computed(() => currentSession.value?.connectionId || '');
const currentPath = computed(() => currentSession.value?.currentPath || '/');
const files = computed(() => currentSession.value?.files || []);
const loading = computed(() => Boolean(currentSession.value?.loading));
const autoConnecting = computed(() => Boolean(currentSession.value?.connecting));
const errorMessage = computed(() => currentSession.value?.errorMessage || '');
const canBrowse = computed(() => Boolean(connectionId.value && !autoConnecting.value));
const canRunConnectedAction = computed(() =>
  Boolean(connectionId.value && !autoConnecting.value && !loading.value && !actionLoading.value),
);
const canRunSelectedAction = computed(() =>
  Boolean(canRunConnectedAction.value && selectedItem.value),
);
const canDownload = computed(() =>
  Boolean(canRunConnectedAction.value && selectedItem.value && !selectedItem.value.isDir),
);
const activeTransferCount = computed(
  () => transferTasks.value.filter((task) => task.status === 'running').length,
);
const activeHostLabel = computed(() => {
  const host = activeHost.value;
  if (!host) return '';
  return `${host.username}@${host.host}`;
});
const statusText = computed(() => {
  if (autoConnecting.value) return '连接中';
  if (loading.value) return '加载中';
  if (connectionId.value) return '已连接';
  return '未连接';
});
const statusNotice = computed(() => {
  if (!activeHost.value) return '请先连接 SSH 主机。';
  if (errorMessage.value) return errorMessage.value;
  if (!canAutoConnect.value) return '当前主机未缓存认证，请重新连接 SSH。';
  if (autoConnecting.value) return '正在连接 SFTP...';
  if (loading.value) return '目录加载中...';
  if (connectionId.value) return `SFTP 已连接：${activeHostLabel.value}`;
  return '正在连接 SFTP...';
});

watch(
  () => ({
    hostId: workspaceStore.activeHost?.id || '',
    credentialVersion: workspaceStore.credentialVersion,
  }),
  () => {
    void syncSftpWithWorkspaceHost();
    void cleanupSessionsWithoutCredential();
  },
  { immediate: true },
);

watch(
  () => activeHost.value?.id || '',
  () => {
    selectedItem.value = null;
    actionMessage.value = '';
  },
);

onMounted(() => {
  void setupTransferProgressListener();
});

onBeforeUnmount(() => {
  disposed = true;
  unlistenTransferProgress?.();
  void closeAllSessions();
});

async function setupTransferProgressListener() {
  const unlisten = await listen<SftpTransferProgressPayload>(
    'sftp-transfer-progress',
    (event: Event<SftpTransferProgressPayload>) => {
      if (disposed) return;
      updateTransferProgress(event.payload);
    },
  );

  if (disposed) {
    unlisten();
    return;
  }

  unlistenTransferProgress = unlisten;
}

function updateTransferProgress(payload: SftpTransferProgressPayload) {
  transferTasks.value = transferTasks.value.map((task) => {
    if (task.id !== payload.transferId) return task;

    return {
      ...task,
      transferredBytes: payload.transferredBytes,
      totalBytes: payload.totalBytes,
      percent: payload.percent,
      status: payload.status,
    };
  });
}

async function syncSftpWithWorkspaceHost() {
  const host = workspaceStore.activeHost;

  if (!host) {
    return;
  }

  const credential = workspaceStore.getCredential(host.id);

  if (!credential?.password) {
    await closeSftpSession(host.id, { silent: true });
    removeSession(host.id);
    return;
  }

  const existing = sessionsByHostId.value[host.id];
  if (existing?.connectionId || existing?.connecting) {
    return;
  }

  await connectSftpWithCredential(host, credential);
}

async function connectSftpWithCredential(
  host: WorkspaceHost,
  credential: WorkspaceCredential,
) {
  const initialPath = sessionsByHostId.value[host.id]?.currentPath || '/';
  upsertSession(host.id, {
    connecting: true,
    loading: false,
    errorMessage: '',
  });

  try {
    const id = await invoke<string>('sftp_connect', {
      payload: {
        host: host.host,
        port: host.port,
        username: host.username,
        password: credential.password,
        privateKeyPath: null,
        passphrase: null,
      },
    });

    if (!workspaceStore.hasCredential(host.id)) {
      await invoke('sftp_close', { connectionId: id });
      return;
    }

    upsertSession(host.id, {
      connectionId: id,
      currentPath: initialPath,
      connecting: false,
      errorMessage: '',
    });
    await loadDir(initialPath, host.id);
  } catch {
    if (!workspaceStore.hasCredential(host.id)) {
      removeSession(host.id);
      return;
    }

    upsertSession(host.id, {
      connectionId: '',
      connecting: false,
      loading: false,
      files: [],
      errorMessage: 'SFTP 自动连接失败，请检查当前 SSH 认证或服务器 SFTP 权限。',
    });
  } finally {
    const latest = sessionsByHostId.value[host.id];
    if (latest?.connecting) {
      upsertSession(host.id, { connecting: false });
    }
  }
}

async function loadDir(path: string, hostId = activeHost.value?.id) {
  if (!hostId) return;

  const session = sessionsByHostId.value[hostId];
  const expectedConnectionId = session?.connectionId;
  if (!expectedConnectionId) return;

  upsertSession(hostId, { loading: true, errorMessage: '' });

  try {
    const items = await invoke<RemoteFileItem[]>('sftp_list_dir', {
      connectionId: expectedConnectionId,
      path,
    });

    const latest = sessionsByHostId.value[hostId];
    if (!latest || latest.connectionId !== expectedConnectionId) return;

    upsertSession(hostId, {
      files: items,
      currentPath: path,
      loading: false,
      errorMessage: '',
    });

    if (activeHost.value?.id === hostId) {
      selectedItem.value = null;
    }
  } catch {
    const latest = sessionsByHostId.value[hostId];
    if (!latest || latest.connectionId !== expectedConnectionId) return;

    upsertSession(hostId, {
      files: [],
      loading: false,
      errorMessage: '目录加载失败，请检查连接状态或目录权限。',
    });
  } finally {
    const latest = sessionsByHostId.value[hostId];
    if (latest?.connectionId === expectedConnectionId && latest.loading) {
      upsertSession(hostId, { loading: false });
    }
  }
}

function getCurrentSession() {
  const hostId = activeHost.value?.id;
  return hostId ? sessionsByHostId.value[hostId] : undefined;
}

function upsertSession(hostId: string, patch: Partial<SftpSessionState>) {
  const current = sessionsByHostId.value[hostId];
  const fallback: SftpSessionState = {
    hostId,
    connectionId: '',
    currentPath: '/',
    files: [],
    loading: false,
    connecting: false,
    errorMessage: '',
  };

  sessionsByHostId.value = {
    ...sessionsByHostId.value,
    [hostId]: {
      ...fallback,
      ...current,
      ...patch,
    },
  };
}

function removeSession(hostId: string) {
  if (!sessionsByHostId.value[hostId]) return;

  const next = { ...sessionsByHostId.value };
  delete next[hostId];
  sessionsByHostId.value = next;
}

async function closeSftpSession(hostId: string, options: { silent?: boolean } = {}) {
  const session = sessionsByHostId.value[hostId];
  const id = session?.connectionId;

  if (session) {
    upsertSession(hostId, {
      connectionId: '',
      files: [],
      loading: false,
      connecting: false,
      errorMessage: options.silent ? session.errorMessage : '',
    });
  }

  if (!id) return;

  try {
    await invoke('sftp_close', { connectionId: id });
  } catch {
    if (!options.silent) {
      upsertSession(hostId, { errorMessage: 'SFTP 关闭失败。' });
    }
  }
}

async function cleanupSessionsWithoutCredential() {
  const hostIds = Object.keys(sessionsByHostId.value);

  for (const hostId of hostIds) {
    if (!workspaceStore.hasCredential(hostId)) {
      await closeSftpSession(hostId, { silent: true });
      removeSession(hostId);
    }
  }
}

async function closeAllSessions() {
  const hostIds = Object.keys(sessionsByHostId.value);

  await Promise.all(hostIds.map((hostId) => closeSftpSession(hostId, { silent: true })));
  sessionsByHostId.value = {};
}

function openItem(file: RemoteFileItem) {
  const hostId = activeHost.value?.id;
  if (!hostId || !file.isDir || getCurrentSession()?.loading) return;
  selectedItem.value = null;
  void loadDir(file.path, hostId);
}

function goParent() {
  const hostId = activeHost.value?.id;
  const path = currentPath.value;
  if (!hostId || path === '/') return;
  selectedItem.value = null;
  void loadDir(getParentPath(path), hostId);
}

function refresh() {
  const host = activeHost.value;
  if (!host) return;

  const session = sessionsByHostId.value[host.id];

  if (!session?.connectionId) {
    void syncSftpWithWorkspaceHost();
    return;
  }

  void loadDir(session.currentPath || '/', host.id);
}

function selectItem(file: RemoteFileItem) {
  selectedItem.value = file;
  actionMessage.value = '';
}

async function uploadFile() {
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;

  const localPath = await open({
    multiple: false,
    directory: false,
  });

  if (!localPath || Array.isArray(localPath)) return;

  const fileName = getLocalFileName(localPath);
  if (!fileName) {
    actionMessage.value = '上传失败，请检查本地文件。';
    return;
  }

  const remotePath = joinRemotePath(snapshot.currentPath, fileName);
  const overwrite = files.value.some((file) => file.path === remotePath);

  if (overwrite) {
    const confirmed = await ask(`远程文件「${fileName}」已存在，是否覆盖？\n\n覆盖后原文件内容不可恢复。`, {
      title: '确认覆盖',
      kind: 'warning',
    });
    if (!confirmed) return;
  }

  const transferId = createTransferTask('upload', fileName);

  await runSftpAction({
    snapshot,
    loadingText: '正在上传...',
    successText: '上传完成。',
    failureText: '上传失败，请检查本地文件或远程目录权限。',
    transferId,
    action: () =>
      invoke('sftp_upload_file', {
        connectionId: snapshot.connectionId,
        localPath,
        remotePath,
        transferId,
      }),
  });
}

async function downloadFile() {
  const item = selectedItem.value;
  if (!item || item.isDir) return;

  const snapshot = await ensureActiveSession();
  if (!snapshot) return;

  const localPath = await save({
    defaultPath: item.name,
  });

  if (!localPath) return;

  const transferId = createTransferTask('download', item.name, item.size);

  await runSftpAction({
    snapshot,
    loadingText: '正在下载...',
    successText: '下载完成。',
    failureText: '下载失败，请检查远程文件或本地保存路径。',
    transferId,
    action: () =>
      invoke('sftp_download_file', {
        connectionId: snapshot.connectionId,
        remotePath: item.path,
        localPath,
        transferId,
      }),
  });
}

async function createDirectory() {
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;

  const dirName = window.prompt('请输入目录名');
  const normalizedName = normalizeRemoteName(dirName);

  if (!normalizedName) return;

  await runSftpAction({
    snapshot,
    loadingText: '正在创建目录...',
    successText: '目录创建完成。',
    failureText: '新建目录失败，请检查远程目录权限。',
    action: () =>
      invoke('sftp_mkdir', {
        connectionId: snapshot.connectionId,
        path: joinRemotePath(snapshot.currentPath, normalizedName),
      }),
  });
}

async function renameItem() {
  const item = selectedItem.value;
  if (!item) return;

  const snapshot = await ensureActiveSession();
  if (!snapshot) return;

  const newName = window.prompt('请输入新名称', item.name);
  const normalizedName = normalizeRemoteName(newName);

  if (!normalizedName || normalizedName === item.name) return;

  await runSftpAction({
    snapshot,
    loadingText: '正在重命名...',
    successText: '重命名完成。',
    failureText: '重命名失败，请检查目标名称或目录权限。',
    action: () =>
      invoke('sftp_rename', {
        connectionId: snapshot.connectionId,
        oldPath: item.path,
        newPath: joinRemotePath(snapshot.currentPath, normalizedName),
      }),
  });
}

async function deleteItem() {
  const item = selectedItem.value;
  if (!item) return;

  const snapshot = await ensureActiveSession();
  if (!snapshot) return;

  const typeText = item.isDir ? '空目录' : '文件';
  const confirmed = await ask(`确认删除${typeText}「${item.name}」？\n\n此操作不可恢复。`, {
    title: '确认删除',
    kind: 'warning',
  });

  if (!confirmed) return;

  await runSftpAction({
    snapshot,
    loadingText: '正在删除...',
    successText: '删除完成。',
    failureText: '删除失败，请确认目录为空或有足够权限。',
    action: () =>
      invoke('sftp_delete', {
        connectionId: snapshot.connectionId,
        path: item.path,
        isDir: item.isDir,
      }),
  });
}

async function ensureActiveSession() {
  let snapshot = getActiveSessionSnapshot();

  if (!snapshot) {
    await syncSftpWithWorkspaceHost();
    snapshot = getActiveSessionSnapshot();
  }

  if (!snapshot) {
    actionMessage.value = activeHost.value ? 'SFTP 尚未连接。' : '请先连接 SSH 主机。';
  }

  return snapshot;
}

function getActiveSessionSnapshot() {
  const host = activeHost.value;
  const session = currentSession.value;

  if (!host || !session?.connectionId) return undefined;

  return {
    hostId: host.id,
    connectionId: session.connectionId,
    currentPath: session.currentPath,
  };
}

async function runSftpAction(options: {
  snapshot: { hostId: string; connectionId: string; currentPath: string };
  loadingText: string;
  successText: string;
  failureText: string;
  transferId?: string;
  action: () => Promise<unknown>;
}) {
  actionLoading.value = true;
  actionStatusText.value = options.loadingText;
  actionMessage.value = '';

  try {
    await options.action();

    const latest = sessionsByHostId.value[options.snapshot.hostId];
    if (!latest || latest.connectionId !== options.snapshot.connectionId) return;

    await loadDir(options.snapshot.currentPath, options.snapshot.hostId);

    if (activeHost.value?.id !== options.snapshot.hostId) return;

    selectedItem.value = null;
    actionMessage.value = options.successText;
  } catch {
    if (options.transferId) {
      markTransferFailed(options.transferId);
    }

    const latest = sessionsByHostId.value[options.snapshot.hostId];
    if (!latest || latest.connectionId !== options.snapshot.connectionId) return;
    if (activeHost.value?.id !== options.snapshot.hostId) return;
    actionMessage.value = options.failureText;
  } finally {
    actionLoading.value = false;
    actionStatusText.value = '';
  }
}

function createTransferTask(type: TransferType, name: string, totalBytes = 0) {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  transferTasks.value = [
    {
      id,
      type,
      name,
      transferredBytes: 0,
      totalBytes,
      percent: 0,
      status: 'running',
      createdAt: Date.now(),
    },
    ...transferTasks.value,
  ].slice(0, 8);
  return id;
}

function markTransferFailed(transferId: string) {
  transferTasks.value = transferTasks.value.map((task) =>
    task.id === transferId
      ? {
          ...task,
          status: 'failed',
        }
      : task,
  );
}

function transferStatusText(status: TransferStatus) {
  if (status === 'success') return '完成';
  if (status === 'failed') return '失败';
  return '传输中';
}

function joinRemotePath(basePath: string, name: string) {
  if (basePath === '/') return `/${name}`;
  return `${basePath.replace(/\/+$/, '')}/${name}`;
}

function normalizeRemoteName(value: string | null) {
  const name = value?.trim() || '';

  if (!name || name === '.' || name === '..' || name.includes('/') || name.includes('\\')) {
    return '';
  }

  return name;
}

function getLocalFileName(path: string) {
  return path.split(/[\\/]/).pop() || '';
}

function getParentPath(path: string) {
  const normalized = path.replace(/\/+$/, '');
  const index = normalized.lastIndexOf('/');

  if (index <= 0) return '/';
  return normalized.slice(0, index);
}

function formatPercent(value: number | undefined) {
  if (value === undefined || Number.isNaN(value)) return '-';
  return `${Math.min(100, Math.max(0, value)).toFixed(1)}%`;
}

function progressWidth(value: number | undefined) {
  if (value === undefined || Number.isNaN(value)) return '0%';
  return `${Math.min(100, Math.max(0, value))}%`;
}

function formatSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${(size / 1024 / 1024 / 1024).toFixed(1)} GB`;
}
</script>

<style scoped>
.sftp-layout {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  padding: 18px;
  gap: 16px;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 64px;
  border: 1px solid #1e293b;
  border-radius: 16px;
  background: #0f172a;
  padding: 14px 18px;
}

.toolbar h2 {
  margin: 0;
  font-size: 20px;
}

.toolbar p {
  margin: 6px 0 0;
  color: #94a3b8;
  font-size: 13px;
}

.status {
  border-radius: 999px;
  background: #334155;
  color: #cbd5e1;
  padding: 6px 12px;
  font-size: 13px;
}

.status--online {
  background: rgba(34, 197, 94, 0.14);
  color: #86efac;
}

.content-grid {
  display: grid;
  min-height: 0;
  flex: 1;
}

.browser-card {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid #1e293b;
  border-radius: 16px;
  background: #0f172a;
}

.status-strip {
  border-bottom: 1px solid #1e293b;
  background: rgba(15, 23, 42, 0.72);
  color: #cbd5e1;
  padding: 9px 12px;
  font-size: 12px;
}

.status-strip--online {
  color: #bbf7d0;
}

.sftp-toolbar-row {
  display: grid;
  align-items: center;
  grid-template-columns: minmax(180px, 420px) auto minmax(0, 1fr);
  gap: 8px;
  border-bottom: 1px solid var(--ls-border);
  background: linear-gradient(180deg, var(--ls-panel-strong), var(--ls-panel-soft));
  padding: 6px 8px;
}

.sftp-path-main {
  min-width: 0;
  width: 100%;
  max-width: 420px;
  height: 28px;
  overflow: hidden;
  border: 1px solid var(--ls-border);
  border-radius: 8px;
  background: var(--ls-panel);
  color: var(--ls-text);
  box-shadow: inset 0 1px 2px rgba(16, 24, 40, 0.08);
  font-size: 12px;
  line-height: 26px;
  padding: 0 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sftp-icon-actions {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.sftp-icon-button {
  display: inline-grid;
  width: 28px;
  height: 28px;
  place-items: center;
  overflow: hidden;
  border: 1px solid var(--ls-border-strong);
  border-radius: 8px;
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-strong));
  color: var(--ls-text);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.24), 0 1px 1px rgba(16, 24, 40, 0.06);
  cursor: pointer;
  font-family: "Segoe UI Symbol", "Arial Unicode MS", Arial, sans-serif;
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  padding: 0;
  text-align: center;
  white-space: nowrap;
}

.sftp-icon-button:hover:not(:disabled) {
  border-color: var(--ls-primary);
  color: var(--ls-primary);
}

.sftp-icon-button--danger:hover:not(:disabled) {
  border-color: var(--ls-danger);
  color: var(--ls-danger);
}

.sftp-icon-button:disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.action-bar {
  display: flex;
  min-height: 48px;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid #1e293b;
  padding: 8px 10px;
}

.action-button {
  height: 32px;
  border: 1px solid #334155;
  border-radius: 8px;
  background: #162033;
  color: #e5e7eb;
  cursor: pointer;
  font-size: 12px;
  white-space: nowrap;
}

.action-button:hover:not(:disabled) {
  border-color: #38bdf8;
  color: #f8fafc;
}

.action-button--danger:hover:not(:disabled) {
  border-color: #f87171;
}

.action-button:disabled,
.ghost-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.action-message {
  min-width: 0;
  max-width: 180px;
  overflow: hidden;
  color: var(--ls-text-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path-bar {
  display: grid;
  grid-template-columns: 76px minmax(0, 1fr) 76px;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid #1e293b;
  padding: 10px;
}

.path-text {
  min-width: 0;
  height: 36px;
  overflow: hidden;
  border: 1px solid #1e293b;
  border-radius: 10px;
  background: #020617;
  color: #e5e7eb;
  line-height: 34px;
  padding: 0 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ghost-button {
  height: 36px;
  border: 1px solid #334155;
  border-radius: 10px;
  background: #1e293b;
  color: #fff;
  cursor: pointer;
}

.error-box {
  margin: 10px 10px 0;
  border: 1px solid rgba(248, 113, 113, 0.36);
  border-radius: 10px;
  background: rgba(127, 29, 29, 0.28);
  color: #fecaca;
  padding: 10px 12px;
  font-size: 13px;
}

.table-wrap {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 10px;
}

.file-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.file-table th,
.file-table td {
  border-bottom: 1px solid #1e293b;
  padding: 9px 10px;
  text-align: left;
}

.file-table th {
  color: #94a3b8;
  font-size: 12px;
  font-weight: 600;
}

.file-table th:nth-child(2),
.file-table td:nth-child(2) {
  width: 96px;
}

.file-table th:nth-child(3),
.file-table td:nth-child(3) {
  width: 120px;
}

.file-table td {
  color: #e5e7eb;
  font-size: 13px;
}

.file-table tbody tr {
  cursor: pointer;
}

.file-table tbody tr:hover {
  background: #111827;
}

.file-row--selected,
.file-row--selected:hover {
  background: rgba(14, 165, 233, 0.18);
}

.name-cell {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.name-cell span:last-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-icon {
  flex: 0 0 auto;
}

.empty-cell {
  height: 180px;
  color: #94a3b8;
  text-align: center;
}

.transfer-panel {
  border-top: 1px solid #1e293b;
  background: rgba(2, 6, 23, 0.36);
}

.transfer-panel__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 34px;
  border-bottom: 1px solid #1e293b;
  color: #cbd5e1;
  padding: 0 12px;
  font-size: 12px;
}

.transfer-list {
  display: grid;
  max-height: 148px;
  overflow: auto;
  padding: 8px 10px;
  gap: 8px;
}

.transfer-task {
  display: grid;
  gap: 5px;
}

.transfer-task__meta,
.transfer-task__foot {
  display: grid;
  min-width: 0;
  align-items: center;
  gap: 8px;
  color: #94a3b8;
  font-size: 12px;
}

.transfer-task__meta {
  grid-template-columns: 42px minmax(0, 1fr) 44px;
}

.transfer-task__meta strong {
  min-width: 0;
  overflow: hidden;
  color: #e5e7eb;
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-task__type {
  color: #93c5fd;
}

.transfer-task__foot {
  grid-template-columns: minmax(0, 1fr) 64px;
}

.transfer-task__foot span:last-child {
  text-align: right;
}

.transfer-progress {
  height: 5px;
  overflow: hidden;
  border-radius: 999px;
  background: #1e293b;
}

.transfer-progress__bar {
  display: block;
  height: 100%;
  transition: width 0.16s ease;
}

.transfer-progress__bar--running {
  background: #38bdf8;
}

.transfer-progress__bar--success {
  background: #22c55e;
}

.transfer-progress__bar--failed {
  background: #ef4444;
}
</style>
