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

        <div class="path-bar">
          <button class="ghost-button" :disabled="!canBrowse || currentPath === '/'" type="button" @click="goParent">
            上级
          </button>
          <div class="path-text">{{ currentPath }}</div>
          <button class="ghost-button" :disabled="autoConnecting || loading" type="button" @click="refresh">
            刷新
          </button>
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
                  :class="{ 'file-row--dir': file.isDir }"
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
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

import { type WorkspaceCredential, type WorkspaceHost, useWorkspaceStore } from '@/stores/workspace';

interface RemoteFileItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

const workspaceStore = useWorkspaceStore();

const activeSftpHostId = ref('');
const connectionId = ref('');
const currentPath = ref('/');
const files = ref<RemoteFileItem[]>([]);
const autoConnecting = ref(false);
const loading = ref(false);
const errorMessage = ref('');

let syncVersion = 0;

const activeHost = computed(() => workspaceStore.activeHost);
const cachedCredential = computed(() =>
  activeHost.value ? workspaceStore.getCredential(activeHost.value.id) : undefined,
);
const canAutoConnect = computed(() =>
  Boolean(activeHost.value && cachedCredential.value?.password),
);
const canBrowse = computed(() => Boolean(connectionId.value && !autoConnecting.value));
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
  if (connectionId.value) return `SFTP 已连接：${activeHostLabel.value}`;
  return '正在连接 SFTP...';
});

watch(
  () => ({
    hostId: workspaceStore.activeHost?.id || '',
    credentialCreatedAt: workspaceStore.activeHost
      ? workspaceStore.getCredential(workspaceStore.activeHost.id)?.createdAt || 0
      : 0,
  }),
  () => {
    void syncSftpWithWorkspaceHost();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  syncVersion += 1;
  void closeSftp({ silent: true });
});

async function syncSftpWithWorkspaceHost() {
  const syncId = ++syncVersion;
  const host = workspaceStore.activeHost;

  if (!host) {
    await closeSftp({ silent: true });
    resetBrowser();
    errorMessage.value = '';
    return;
  }

  const credential = workspaceStore.getCredential(host.id);

  if (!credential?.password) {
    await closeSftp({ silent: true });
    resetBrowser();
    errorMessage.value = '';
    return;
  }

  if (connectionId.value && activeSftpHostId.value === host.id) {
    return;
  }

  await closeSftp({ silent: true });
  if (syncId !== syncVersion) return;
  await connectSftpWithCredential(host, credential, syncId);
}

async function connectSftpWithCredential(
  host: WorkspaceHost,
  credential: WorkspaceCredential,
  syncId: number,
) {
  autoConnecting.value = true;
  errorMessage.value = '';

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

    if (syncId !== syncVersion) {
      await invoke('sftp_close', { connectionId: id });
      return;
    }

    connectionId.value = id;
    activeSftpHostId.value = host.id;
    currentPath.value = '/';
    await loadDir('/', id);
  } catch {
    if (syncId !== syncVersion) return;
    connectionId.value = '';
    activeSftpHostId.value = '';
    files.value = [];
    errorMessage.value = 'SFTP 自动连接失败，请检查当前 SSH 认证或服务器 SFTP 权限。';
  } finally {
    if (syncId === syncVersion) {
      autoConnecting.value = false;
    }
  }
}

async function loadDir(path: string, expectedConnectionId = connectionId.value) {
  if (!expectedConnectionId) return;

  loading.value = true;
  errorMessage.value = '';

  try {
    const items = await invoke<RemoteFileItem[]>('sftp_list_dir', {
      connectionId: expectedConnectionId,
      path,
    });

    if (connectionId.value !== expectedConnectionId) return;

    files.value = items;
    currentPath.value = path;
  } catch {
    if (connectionId.value !== expectedConnectionId) return;

    files.value = [];
    errorMessage.value = '目录加载失败，请检查连接状态或目录权限。';
  } finally {
    if (connectionId.value === expectedConnectionId) {
      loading.value = false;
    }
  }
}

async function closeSftp(options: { silent?: boolean } = {}) {
  const id = connectionId.value;
  connectionId.value = '';
  activeSftpHostId.value = '';
  files.value = [];
  currentPath.value = '/';

  if (!options.silent) {
    errorMessage.value = '';
  }

  if (!id) return;

  try {
    await invoke('sftp_close', { connectionId: id });
  } catch {
    if (!options.silent) {
      errorMessage.value = 'SFTP 关闭失败。';
    }
  }
}

function resetBrowser() {
  connectionId.value = '';
  activeSftpHostId.value = '';
  files.value = [];
  currentPath.value = '/';
  autoConnecting.value = false;
  loading.value = false;
}

function openItem(file: RemoteFileItem) {
  if (!file.isDir || loading.value) return;
  void loadDir(file.path);
}

function goParent() {
  if (currentPath.value === '/') return;
  void loadDir(getParentPath(currentPath.value));
}

function refresh() {
  if (!connectionId.value) {
    void syncSftpWithWorkspaceHost();
    return;
  }

  void loadDir(currentPath.value);
}

function getParentPath(path: string) {
  const normalized = path.replace(/\/+$/, '');
  const index = normalized.lastIndexOf('/');

  if (index <= 0) return '/';
  return normalized.slice(0, index);
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

.ghost-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
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

.file-row--dir {
  cursor: pointer;
}

.file-row--dir:hover {
  background: #111827;
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
</style>
