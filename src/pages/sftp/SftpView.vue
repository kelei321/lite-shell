<template>
  <section class="sftp-layout">
    <header class="toolbar">
      <div>
        <h2>SFTP 文件</h2>
        <p>复用主机列表，密码只用于本次 SFTP 连接。</p>
      </div>
      <div class="status" :class="{ 'status--online': connectionId }">
        {{ statusText }}
      </div>
    </header>

    <div class="content-grid">
      <aside class="host-panel">
        <form class="connect-card" @submit.prevent="connectSftp">
          <div class="panel-title-row">
            <h3>连接信息</h3>
            <button class="tiny-button" :disabled="!connectionId" type="button" @click="closeSftp()">
              断开
            </button>
          </div>

          <label>
            <span>名称</span>
            <input v-model.trim="form.name" autocomplete="off" placeholder="生产服务器" />
          </label>

          <label>
            <span>主机</span>
            <input v-model.trim="form.host" autocomplete="off" placeholder="127.0.0.1" />
          </label>

          <label>
            <span>端口</span>
            <input v-model.number="form.port" min="1" max="65535" type="number" />
          </label>

          <label>
            <span>用户名</span>
            <input v-model.trim="form.username" autocomplete="username" placeholder="root" />
          </label>

          <label>
            <span>密码</span>
            <input v-model="form.password" autocomplete="current-password" type="password" />
          </label>

          <button class="primary-button" :disabled="!canConnect || connecting" type="submit">
            {{ connecting ? '连接中...' : '连接 SFTP' }}
          </button>
        </form>

        <section class="host-list-card">
          <h3>主机列表</h3>
          <p v-if="hostStore.sortedHosts.length === 0" class="empty-tip">
            暂无主机，请先在终端页保存主机。
          </p>

          <button
            v-for="host in hostStore.sortedHosts"
            :key="host.id"
            class="host-item"
            :class="{ 'host-item--active': host.id === form.id }"
            type="button"
            @click="selectHost(host)"
          >
            <span class="host-item__name">{{ host.name }}</span>
            <span class="host-item__meta">{{ host.username }}@{{ host.host }}:{{ host.port }}</span>
          </button>
        </section>
      </aside>

      <section class="browser-card">
        <div class="path-bar">
          <button class="ghost-button" :disabled="!canBrowse || currentPath === '/'" type="button" @click="goParent">
            上级
          </button>
          <div class="path-text">{{ currentPath }}</div>
          <button class="ghost-button" :disabled="!canBrowse || loading" type="button" @click="refresh">
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
                <td colspan="3" class="empty-cell">选择主机并输入密码后连接。</td>
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
import { computed, onBeforeUnmount, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

import { type HostProfile, useHostStore } from '@/stores/hosts';

interface RemoteFileItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

const hostStore = useHostStore();

const form = reactive({
  id: '',
  name: '',
  host: '',
  port: 22,
  username: '',
  password: '',
});

const connectionId = ref('');
const currentPath = ref('/');
const files = ref<RemoteFileItem[]>([]);
const connecting = ref(false);
const loading = ref(false);
const errorMessage = ref('');

const canConnect = computed(() =>
  Boolean(form.host.trim() && form.username.trim() && form.password && !loading.value),
);
const canBrowse = computed(() => Boolean(connectionId.value && !connecting.value));
const statusText = computed(() => {
  if (connecting.value) return '连接中';
  if (loading.value) return '加载中';
  if (connectionId.value) return '已连接';
  return '未连接';
});

onBeforeUnmount(() => {
  void closeSftp();
});

function selectHost(host: HostProfile) {
  form.id = host.id;
  form.name = host.name;
  form.host = host.host;
  form.port = host.port;
  form.username = host.username;
  form.password = '';
  errorMessage.value = '';
}

async function connectSftp() {
  if (!canConnect.value || connecting.value) return;

  const password = form.password;
  connecting.value = true;
  errorMessage.value = '';

  try {
    await closeSftp({ silent: true });

    const id = await invoke<string>('sftp_connect', {
      payload: {
        host: form.host,
        port: form.port,
        username: form.username,
        password,
        privateKeyPath: null,
        passphrase: null,
      },
    });

    connectionId.value = id;
    currentPath.value = '/';
    form.password = '';
    if (form.id) hostStore.touchHost(form.id);
    await loadDir('/');
  } catch (error) {
    errorMessage.value = `SFTP 连接失败：${String(error)}`;
    connectionId.value = '';
    files.value = [];
  } finally {
    connecting.value = false;
  }
}

async function loadDir(path: string) {
  if (!connectionId.value) return;

  loading.value = true;
  errorMessage.value = '';

  try {
    const items = await invoke<RemoteFileItem[]>('sftp_list_dir', {
      connectionId: connectionId.value,
      path,
    });

    files.value = items;
    currentPath.value = path;
  } catch (error) {
    errorMessage.value = `目录加载失败：${String(error)}`;
  } finally {
    loading.value = false;
  }
}

async function closeSftp(options: { silent?: boolean } = {}) {
  const id = connectionId.value;
  connectionId.value = '';
  files.value = [];
  currentPath.value = '/';

  if (!id) return;

  try {
    await invoke('sftp_close', { connectionId: id });
  } catch (error) {
    if (!options.silent) {
      errorMessage.value = `SFTP 关闭失败：${String(error)}`;
    }
  }
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

.toolbar h2,
.host-list-card h3,
.panel-title-row h3 {
  margin: 0;
}

.toolbar h2 {
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
  grid-template-columns: 320px minmax(0, 1fr);
  min-height: 0;
  flex: 1;
  gap: 16px;
}

.host-panel {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
}

.connect-card,
.host-list-card,
.browser-card {
  border: 1px solid #1e293b;
  border-radius: 16px;
  background: #0f172a;
}

.connect-card,
.host-list-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.host-list-card {
  min-height: 0;
  overflow: auto;
}

.panel-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.connect-card label {
  display: grid;
  gap: 6px;
}

.connect-card span {
  color: #94a3b8;
  font-size: 12px;
}

.connect-card input {
  width: 100%;
  height: 36px;
  border: 1px solid #334155;
  border-radius: 10px;
  outline: none;
  background: #020617;
  color: #e5e7eb;
  padding: 0 10px;
}

.connect-card input:focus {
  border-color: #2563eb;
}

.primary-button,
.ghost-button,
.tiny-button,
.host-item {
  border-radius: 10px;
  color: #fff;
  cursor: pointer;
}

.primary-button,
.ghost-button {
  height: 36px;
}

.primary-button {
  background: #2563eb;
}

.ghost-button,
.tiny-button {
  border: 1px solid #334155;
  background: #1e293b;
}

.tiny-button {
  height: 28px;
  padding: 0 10px;
  color: #cbd5e1;
}

.primary-button:disabled,
.ghost-button:disabled,
.tiny-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.empty-tip {
  color: #94a3b8;
  font-size: 13px;
}

.host-item {
  display: grid;
  gap: 4px;
  border: 1px solid #1e293b;
  background: #111827;
  padding: 10px;
  text-align: left;
}

.host-item:hover,
.host-item--active {
  border-color: #2563eb;
}

.host-item__name {
  color: #f8fafc;
  font-size: 14px;
}

.host-item__meta {
  color: #94a3b8;
  font-size: 12px;
}

.browser-card {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
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
