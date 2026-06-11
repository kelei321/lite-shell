<template>
  <section class="sftp-layout" @dragenter.prevent="showDropOverlay" @dragover.prevent="showDropOverlay" @dragleave.prevent="hideDropOverlay" @drop.prevent="handleDropUpload">
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
          <span>{{ statusNotice }}</span>
          <button v-if="clipboard" class="clipboard-chip" type="button" title="清空剪贴板" @click="clearClipboard">
            剪贴板：{{ clipboard.mode === 'copy' ? '复制' : '剪切' }} {{ clipboard.items.length }} 项 ×
          </button>
        </div>

        <div class="sftp-toolbar-row">
          <div class="sftp-path-main" :title="currentPath">{{ currentPath }}</div>
          <div class="sftp-icon-actions" aria-label="SFTP 操作">
            <button class="sftp-icon-button" :disabled="!canBrowse || currentPath === '/'" title="上级" type="button" aria-label="上级" @click="goParent">↖</button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="上传文件" type="button" aria-label="上传文件" @click="uploadFile">↑</button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="上传文件夹" type="button" aria-label="上传文件夹" @click="uploadDirectory">⇈</button>
            <button class="sftp-icon-button" :disabled="!canDownloadSelected" title="下载" type="button" aria-label="下载" @click="downloadSelected">↓</button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="新建文件" type="button" aria-label="新建文件" @click="openCreateDialog('file')">□</button>
            <button class="sftp-icon-button" :disabled="!canRunConnectedAction" title="新建文件夹" type="button" aria-label="新建文件夹" @click="openCreateDialog('dir')">⊞</button>
            <button class="sftp-icon-button" :disabled="!canRename" title="重命名" type="button" aria-label="重命名" @click="renameItem">✎</button>
            <button class="sftp-icon-button" :disabled="!hasSelection" title="复制" type="button" aria-label="复制" @click="copySelected">⧉</button>
            <button class="sftp-icon-button" :disabled="!hasSelection" title="剪切" type="button" aria-label="剪切" @click="cutSelected">✂</button>
            <button class="sftp-icon-button" :disabled="!canPaste" title="粘贴" type="button" aria-label="粘贴" @click="pasteClipboard">▣</button>
            <button class="sftp-icon-button sftp-icon-button--danger" :disabled="!hasSelection" title="删除" type="button" aria-label="删除" @click="openDeleteDialog">⌫</button>
            <button class="sftp-icon-button" :disabled="autoConnecting || loading || actionLoading" title="刷新" type="button" aria-label="刷新" @click="refresh">⟳</button>
            <button class="sftp-icon-button" :disabled="!canShowProperties" title="属性" type="button" aria-label="属性" @click="openPropertiesDialog('properties')">ⓘ</button>
            <button class="sftp-icon-button" :disabled="!canShowProperties" title="权限" type="button" aria-label="权限" @click="openPropertiesDialog('chmod')">🔒</button>
          </div>
          <span v-if="actionLoading" class="action-message">{{ actionStatusText }}</span>
          <span v-else-if="actionMessage" class="action-message">{{ actionMessage }}</span>
        </div>

        <div v-if="errorMessage" class="error-box">{{ errorMessage }}</div>

        <div class="table-wrap" @click.self="clearSelection" @contextmenu.prevent="openContextMenu($event)">
          <div v-if="isDropActive" class="drop-overlay">
            <div>
              <strong>☁</strong>
              <span>拖拽文件或文件夹到此处上传</span>
            </div>
          </div>

          <table class="file-table">
            <thead>
              <tr>
                <th class="check-column"><input :checked="isAllSelected" :disabled="files.length === 0" type="checkbox" @change="toggleSelectAll" /></th>
                <th>名称</th>
                <th>类型</th>
                <th>大小</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="!connectionId">
                <td colspan="4" class="empty-cell">{{ statusNotice }}</td>
              </tr>
              <tr v-else-if="loading">
                <td colspan="4" class="empty-cell">目录加载中...</td>
              </tr>
              <tr v-else-if="files.length === 0">
                <td colspan="4" class="empty-cell">当前目录为空。</td>
              </tr>
              <template v-else>
                <tr
                  v-for="file in files"
                  :key="file.path"
                  :class="{ 'file-row--dir': file.isDir, 'file-row--selected': isSelected(file.path), 'file-row--anchor': lastSelectedPath === file.path }"
                  @click="handleRowClick($event, file)"
                  @dblclick="openItem(file)"
                  @contextmenu.prevent="openContextMenu($event, file)"
                >
                  <td class="check-column"><input :checked="isSelected(file.path)" type="checkbox" @click.stop="toggleItem(file)" /></td>
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

        <div class="selection-bar">
          <span>{{ selectionSummary }}</span>
          <span>{{ files.length ? `共 ${files.length} 项` : '' }}</span>
        </div>

        <section class="transfer-panel" :class="{ 'transfer-panel--empty': transferTasks.length === 0 }">
          <div class="transfer-panel__head">
            <button class="transfer-title-button" type="button" @click="isTransferPanelExpanded = !isTransferPanelExpanded">
              <strong>传输队列（{{ transferTasks.length }}）</strong>
              <span>{{ isTransferPanelExpanded ? '⌄' : '›' }}</span>
            </button>
            <div class="transfer-actions">
              <button :disabled="activeTransferCount === 0" type="button" @click="pauseAllTransfers">全部暂停</button>
              <button :disabled="!hasCompletedTransfer" type="button" @click="clearFinishedTransfers">清除已完成</button>
            </div>
          </div>
          <div v-if="isTransferPanelExpanded && transferTasks.length" class="transfer-list transfer-list--table">
            <div class="transfer-row transfer-row--head">
              <span>类型</span><span>文件名</span><span>目标路径</span><span>大小</span><span>进度</span><span>状态</span><span>操作</span>
            </div>
            <div v-for="task in transferTasks" :key="task.id" class="transfer-row">
              <span class="transfer-task__type">{{ transferTypeText(task.type) }}</span>
              <strong>{{ task.name }}</strong>
              <span>{{ task.targetPath || '-' }}</span>
              <span>{{ task.totalBytes ? formatSize(task.totalBytes) : '-' }}</span>
              <span class="transfer-progress-cell"><i :class="`transfer-progress__bar transfer-progress__bar--${task.status}`" :style="{ width: progressWidth(task.percent) }"></i><b>{{ formatPercent(task.percent) }}</b></span>
              <span :class="`transfer-status transfer-status--${task.status}`">{{ transferStatusText(task.status) }}</span>
              <span class="transfer-row-actions"><button v-if="task.status === 'failed'" type="button" @click="retryTransfer(task)">重试</button><button type="button" @click="cancelTransfer(task)">×</button></span>
            </div>
          </div>
        </section>

        <ul v-if="contextMenu.visible" class="context-menu" :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }">
          <li><button :disabled="!canOpenSelected" type="button" @click="runMenuAction(openSelected)">打开</button></li>
          <li><button :disabled="!canDownloadSelected" type="button" @click="runMenuAction(downloadSelected)">下载</button></li>
          <li><button :disabled="!canRunConnectedAction" type="button" @click="runMenuAction(uploadFile)">上传文件到当前目录</button></li>
          <li><button :disabled="!canRunConnectedAction" type="button" @click="runMenuAction(uploadDirectory)">上传文件夹到当前目录</button></li>
          <li class="context-menu__separator"></li>
          <li><button :disabled="!canRunConnectedAction" type="button" @click="runMenuAction(() => openCreateDialog('file'))">新建文件</button></li>
          <li><button :disabled="!canRunConnectedAction" type="button" @click="runMenuAction(() => openCreateDialog('dir'))">新建文件夹</button></li>
          <li><button :disabled="!canRename" type="button" @click="runMenuAction(renameItem)">重命名</button></li>
          <li class="context-menu__separator"></li>
          <li><button :disabled="!hasSelection" type="button" @click="runMenuAction(copySelected)">复制</button></li>
          <li><button :disabled="!hasSelection" type="button" @click="runMenuAction(cutSelected)">剪切</button></li>
          <li><button :disabled="!canPaste" type="button" @click="runMenuAction(pasteClipboard)">粘贴</button></li>
          <li><button class="danger" :disabled="!hasSelection" type="button" @click="runMenuAction(openDeleteDialog)">删除</button></li>
          <li class="context-menu__separator"></li>
          <li><button :disabled="!canShowProperties" type="button" @click="runMenuAction(() => openPropertiesDialog('chmod'))">权限</button></li>
          <li><button :disabled="!canShowProperties" type="button" @click="runMenuAction(() => openPropertiesDialog('properties'))">属性</button></li>
          <li><button :disabled="!canRunConnectedAction" type="button" @click="runMenuAction(refresh)">刷新</button></li>
        </ul>

        <div v-if="createDialog.visible" class="sftp-modal-mask" @click.self="closeCreateDialog">
          <section class="sftp-modal sftp-modal--small">
            <header><strong>新建</strong><button type="button" @click="closeCreateDialog">×</button></header>
            <div class="segmented-control">
              <button :class="{ active: createDialog.kind === 'file' }" type="button" @click="createDialog.kind = 'file'">新建文件</button>
              <button :class="{ active: createDialog.kind === 'dir' }" type="button" @click="createDialog.kind = 'dir'">新建文件夹</button>
            </div>
            <label class="modal-field"><span>名称</span><input v-model.trim="createDialog.name" :placeholder="createDialog.kind === 'file' ? '请输入文件名' : '请输入文件夹名'" /></label>
            <p v-if="createDialog.error" class="modal-error">{{ createDialog.error }}</p>
            <footer><button type="button" @click="closeCreateDialog">取消</button><button class="primary" type="button" @click="confirmCreateEntry">确定</button></footer>
          </section>
        </div>

        <div v-if="deleteDialog.visible" class="sftp-modal-mask" @click.self="closeDeleteDialog">
          <section class="sftp-modal sftp-modal--small">
            <header><strong>确认删除</strong><button type="button" @click="closeDeleteDialog">×</button></header>
            <div class="delete-warning"><b>!</b><div><h4>将递归删除选中的 {{ selectedItems.length }} 项内容</h4><p>其中包含的文件和子目录也将被删除，且无法恢复。</p></div></div>
            <footer><button type="button" @click="closeDeleteDialog">取消</button><button class="danger" type="button" @click="confirmDeleteSelected">确认删除</button></footer>
          </section>
        </div>

        <div v-if="propertiesDialog.visible && propertiesDialog.item" class="sftp-modal-mask" @click.self="closePropertiesDialog">
          <section class="sftp-modal sftp-modal--properties">
            <header><strong>文件属性</strong><button type="button" @click="closePropertiesDialog">×</button></header>
            <p v-if="propertiesDialog.loading" class="compact-hint">正在读取远端属性...</p>
            <p v-else-if="propertiesDialog.error" class="modal-error">{{ propertiesDialog.error }}</p>
            <dl class="properties-list">
              <div><dt>名称</dt><dd>{{ currentProperty.name }}</dd></div>
              <div><dt>路径</dt><dd>{{ currentProperty.path }}</dd></div>
              <div><dt>类型</dt><dd>{{ currentProperty.isDir ? '文件夹' : '文件' }}</dd></div>
              <div><dt>大小</dt><dd>{{ currentProperty.isDir ? '-' : formatSize(currentProperty.size) }}</dd></div>
              <div><dt>权限</dt><dd>{{ currentProperty.permissions || '待读取' }}</dd></div>
              <div><dt>修改时间</dt><dd>{{ formatModifiedAt(currentProperty.modifiedAt) }}</dd></div>
              <div><dt>所有者</dt><dd>{{ formatOwner(currentProperty) }}</dd></div>
              <div><dt>是否目录</dt><dd>{{ currentProperty.isDir ? '是' : '否' }}</dd></div>
            </dl>
            <section class="chmod-box">
              <h4>权限设置</h4>
              <label class="modal-field modal-field--inline"><span>八进制</span><input v-model.trim="chmodDialog.mode" placeholder="例如：755" maxlength="4" /></label>
              <p class="compact-hint">保存权限后将刷新当前目录以更新显示。</p>
            </section>
            <footer><button type="button" @click="closePropertiesDialog">取消</button><button class="primary" type="button" @click="savePermissions">保存</button></footer>
          </section>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { type Event, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';

import { type WorkspaceCredential, type WorkspaceHost, useWorkspaceStore } from '@/stores/workspace';

interface RemoteFileItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

interface RemoteFileStat extends RemoteFileItem {
  permissions: string;
  uid?: number;
  gid?: number;
  modifiedAt?: number;
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

type TransferStatus = 'queued' | 'running' | 'success' | 'failed' | 'cancelled';
type TransferType = 'upload' | 'download' | 'copy' | 'delete';
type ClipboardMode = 'copy' | 'cut';
type CreateEntryKind = 'file' | 'dir';

interface TransferTask {
  id: string;
  type: TransferType;
  name: string;
  targetPath: string;
  transferredBytes: number;
  totalBytes: number;
  percent: number;
  status: TransferStatus;
  createdAt: number;
}

interface ClipboardState {
  mode: ClipboardMode;
  sourceHostId: string;
  items: RemoteFileItem[];
}

const workspaceStore = useWorkspaceStore();

const sessionsByHostId = shallowRef<Record<string, SftpSessionState>>({});
const selectedPaths = ref<Set<string>>(new Set());
const lastSelectedPath = ref('');
const clipboard = ref<ClipboardState | null>(null);
const transferTasks = ref<TransferTask[]>([]);
const actionLoading = ref(false);
const actionMessage = ref('');
const actionStatusText = ref('');
const isDropActive = ref(false);
const isTransferPanelExpanded = ref(true);
const contextMenu = reactive({ visible: false, x: 0, y: 0 });
const createDialog = reactive({ visible: false, kind: 'file' as CreateEntryKind, name: '', error: '' });
const deleteDialog = reactive({ visible: false });
const propertiesDialog = reactive({ visible: false, item: null as RemoteFileItem | null, stat: null as RemoteFileStat | null, loading: false, error: '', mode: 'properties' as 'properties' | 'chmod' });
const chmodDialog = reactive({ mode: '755' });
let disposed = false;
let unlistenTransferProgress: UnlistenFn | undefined;

const activeHost = computed(() => workspaceStore.activeHost);
const cachedCredential = computed(() =>
  activeHost.value ? workspaceStore.getCredential(activeHost.value.id) : undefined,
);
const canAutoConnect = computed(() => Boolean(activeHost.value && cachedCredential.value?.password));
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
const canRunConnectedAction = computed(() => Boolean(connectionId.value && !autoConnecting.value && !loading.value && !actionLoading.value));
const selectedItems = computed(() => files.value.filter((file) => selectedPaths.value.has(file.path)));
const hasSelection = computed(() => selectedItems.value.length > 0);
const canRename = computed(() => canRunConnectedAction.value && selectedItems.value.length === 1);
const canShowProperties = computed(() => canRunConnectedAction.value && selectedItems.value.length === 1);
const canOpenSelected = computed(() => canRunConnectedAction.value && selectedItems.value.length === 1);
const canDownloadSelected = computed(() => canRunConnectedAction.value && hasSelection.value);
const canPaste = computed(() => Boolean(canRunConnectedAction.value && clipboard.value && clipboard.value.sourceHostId === activeHost.value?.id));
const isAllSelected = computed(() => files.value.length > 0 && files.value.every((file) => selectedPaths.value.has(file.path)));
const hasCompletedTransfer = computed(() => transferTasks.value.some((task) => task.status === 'success' || task.status === 'failed' || task.status === 'cancelled'));
const activeTransferCount = computed(() => transferTasks.value.filter((task) => task.status === 'running').length);
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
const currentProperty = computed<RemoteFileStat>(() => {
  const fallback = propertiesDialog.item || { name: '-', path: '-', isDir: false, size: 0 };
  return propertiesDialog.stat || { ...fallback, permissions: '', uid: undefined, gid: undefined, modifiedAt: undefined };
});
const selectionSummary = computed(() => {
  if (!selectedItems.value.length) return '未选择项目';
  const dirCount = selectedItems.value.filter((item) => item.isDir).length;
  const fileCount = selectedItems.value.length - dirCount;
  const size = selectedItems.value.reduce((total, item) => total + (item.isDir ? 0 : item.size), 0);
  const parts = [`已选 ${selectedItems.value.length} 项`];
  if (fileCount) parts.push(`${fileCount} 个文件`);
  if (dirCount) parts.push(`${dirCount} 个文件夹`);
  if (size) parts.push(formatSize(size));
  return parts.join('，');
});

watch(
  () => ({ hostId: workspaceStore.activeHost?.id || '', credentialVersion: workspaceStore.credentialVersion }),
  () => {
    void syncSftpWithWorkspaceHost();
    void cleanupSessionsWithoutCredential();
  },
  { immediate: true },
);

watch(
  () => activeHost.value?.id || '',
  () => {
    clearSelection();
    clearClipboard();
    actionMessage.value = '';
  },
);

onMounted(() => {
  void setupTransferProgressListener();
  window.addEventListener('click', closeContextMenu);
  window.addEventListener('keydown', handleShortcut);
});

onBeforeUnmount(() => {
  disposed = true;
  unlistenTransferProgress?.();
  window.removeEventListener('click', closeContextMenu);
  window.removeEventListener('keydown', handleShortcut);
  void closeAllSessions();
});

async function setupTransferProgressListener() {
  const unlisten = await listen<SftpTransferProgressPayload>('sftp-transfer-progress', (event: Event<SftpTransferProgressPayload>) => {
    if (disposed) return;
    updateTransferProgress(event.payload);
  });

  if (disposed) {
    unlisten();
    return;
  }

  unlistenTransferProgress = unlisten;
}

function updateTransferProgress(payload: SftpTransferProgressPayload) {
  transferTasks.value = transferTasks.value.map((task) => {
    if (task.id !== payload.transferId) return task;
    return { ...task, transferredBytes: payload.transferredBytes, totalBytes: payload.totalBytes, percent: payload.percent, status: payload.status };
  });
}

async function syncSftpWithWorkspaceHost() {
  const host = workspaceStore.activeHost;
  if (!host) return;
  const credential = workspaceStore.getCredential(host.id);

  if (!credential?.password) {
    await closeSftpSession(host.id, { silent: true });
    removeSession(host.id);
    return;
  }

  const existing = sessionsByHostId.value[host.id];
  if (existing?.connectionId || existing?.connecting) return;
  await connectSftpWithCredential(host, credential);
}

async function connectSftpWithCredential(host: WorkspaceHost, credential: WorkspaceCredential) {
  const initialPath = sessionsByHostId.value[host.id]?.currentPath || '/';
  upsertSession(host.id, { connecting: true, loading: false, errorMessage: '' });

  try {
    const id = await invoke<string>('sftp_connect', {
      payload: { host: host.host, port: host.port, username: host.username, password: credential.password, privateKeyPath: null, passphrase: null },
    });

    if (!workspaceStore.hasCredential(host.id)) {
      await invoke('sftp_close', { connectionId: id });
      return;
    }

    upsertSession(host.id, { connectionId: id, currentPath: initialPath, connecting: false, errorMessage: '' });
    await loadDir(initialPath, host.id);
  } catch {
    if (!workspaceStore.hasCredential(host.id)) {
      removeSession(host.id);
      return;
    }
    upsertSession(host.id, { connectionId: '', connecting: false, loading: false, files: [], errorMessage: 'SFTP 自动连接失败，请检查当前 SSH 认证或服务器 SFTP 权限。' });
  } finally {
    const latest = sessionsByHostId.value[host.id];
    if (latest?.connecting) upsertSession(host.id, { connecting: false });
  }
}

async function loadDir(path: string, hostId = activeHost.value?.id) {
  if (!hostId) return;
  const session = sessionsByHostId.value[hostId];
  const expectedConnectionId = session?.connectionId;
  if (!expectedConnectionId) return;
  upsertSession(hostId, { loading: true, errorMessage: '' });

  try {
    const items = await invoke<RemoteFileItem[]>('sftp_list_dir', { connectionId: expectedConnectionId, path });
    const latest = sessionsByHostId.value[hostId];
    if (!latest || latest.connectionId !== expectedConnectionId) return;
    upsertSession(hostId, { files: items, currentPath: path, loading: false, errorMessage: '' });
    if (activeHost.value?.id === hostId) clearSelection();
  } catch {
    const latest = sessionsByHostId.value[hostId];
    if (!latest || latest.connectionId !== expectedConnectionId) return;
    upsertSession(hostId, { files: [], loading: false, errorMessage: '目录加载失败，请检查连接状态或目录权限。' });
  } finally {
    const latest = sessionsByHostId.value[hostId];
    if (latest?.connectionId === expectedConnectionId && latest.loading) upsertSession(hostId, { loading: false });
  }
}

function getCurrentSession() {
  const hostId = activeHost.value?.id;
  return hostId ? sessionsByHostId.value[hostId] : undefined;
}

function upsertSession(hostId: string, patch: Partial<SftpSessionState>) {
  const current = sessionsByHostId.value[hostId];
  const fallback: SftpSessionState = { hostId, connectionId: '', currentPath: '/', files: [], loading: false, connecting: false, errorMessage: '' };
  sessionsByHostId.value = { ...sessionsByHostId.value, [hostId]: { ...fallback, ...current, ...patch } };
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
  if (session) upsertSession(hostId, { connectionId: '', files: [], loading: false, connecting: false, errorMessage: options.silent ? session.errorMessage : '' });
  if (!id) return;

  try {
    await invoke('sftp_close', { connectionId: id });
  } catch {
    if (!options.silent) upsertSession(hostId, { errorMessage: 'SFTP 关闭失败。' });
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

function handleShortcut(event: KeyboardEvent) {
  if (!canRunConnectedAction.value) return;
  const isModifier = event.ctrlKey || event.metaKey;
  if (!isModifier) return;
  if (event.key.toLowerCase() === 'a') {
    event.preventDefault();
    selectAll();
  } else if (event.key.toLowerCase() === 'c') {
    copySelected();
  } else if (event.key.toLowerCase() === 'x') {
    cutSelected();
  } else if (event.key.toLowerCase() === 'v') {
    void pasteClipboard();
  }
}

function isSelected(path: string) {
  return selectedPaths.value.has(path);
}

function setSelectedPaths(paths: string[]) {
  selectedPaths.value = new Set(paths);
}

function clearSelection() {
  setSelectedPaths([]);
  lastSelectedPath.value = '';
}

function selectAll() {
  setSelectedPaths(files.value.map((file) => file.path));
  lastSelectedPath.value = files.value.at(-1)?.path || '';
}

function toggleSelectAll() {
  if (isAllSelected.value) clearSelection();
  else selectAll();
}

function toggleItem(file: RemoteFileItem) {
  const next = new Set(selectedPaths.value);
  if (next.has(file.path)) next.delete(file.path);
  else next.add(file.path);
  selectedPaths.value = next;
  lastSelectedPath.value = file.path;
}

function handleRowClick(event: MouseEvent, file: RemoteFileItem) {
  if (event.shiftKey && lastSelectedPath.value) {
    selectRange(file.path);
    return;
  }

  if (event.ctrlKey || event.metaKey) {
    toggleItem(file);
    return;
  }

  setSelectedPaths([file.path]);
  lastSelectedPath.value = file.path;
}

function selectRange(path: string) {
  const paths = files.value.map((file) => file.path);
  const start = paths.indexOf(lastSelectedPath.value);
  const end = paths.indexOf(path);
  if (start < 0 || end < 0) {
    setSelectedPaths([path]);
    lastSelectedPath.value = path;
    return;
  }
  const [from, to] = start <= end ? [start, end] : [end, start];
  setSelectedPaths(paths.slice(from, to + 1));
}

function openContextMenu(event: MouseEvent, file?: RemoteFileItem) {
  event.preventDefault();
  if (file && !isSelected(file.path)) {
    setSelectedPaths([file.path]);
    lastSelectedPath.value = file.path;
  }
  contextMenu.visible = true;
  contextMenu.x = event.clientX;
  contextMenu.y = event.clientY;
}

function closeContextMenu() {
  contextMenu.visible = false;
}

function runMenuAction(action: () => void | Promise<void>) {
  closeContextMenu();
  void action();
}

function openItem(file: RemoteFileItem) {
  const hostId = activeHost.value?.id;
  if (!hostId || getCurrentSession()?.loading) return;
  if (file.isDir) {
    clearSelection();
    void loadDir(file.path, hostId);
  } else {
    setSelectedPaths([file.path]);
    void downloadSelected();
  }
}

function openSelected() {
  const item = selectedItems.value[0];
  if (item) openItem(item);
}

function goParent() {
  const hostId = activeHost.value?.id;
  const path = currentPath.value;
  if (!hostId || path === '/') return;
  clearSelection();
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

function copySelected() {
  setClipboard('copy');
}

function cutSelected() {
  setClipboard('cut');
}

function setClipboard(mode: ClipboardMode) {
  const hostId = activeHost.value?.id;
  if (!hostId || !selectedItems.value.length) return;
  clipboard.value = { mode, sourceHostId: hostId, items: selectedItems.value.map((item) => ({ ...item })) };
  actionMessage.value = `已${mode === 'copy' ? '复制' : '剪切'} ${selectedItems.value.length} 项。`;
}

function clearClipboard() {
  clipboard.value = null;
}

async function pasteClipboard() {
  const snapshot = await ensureActiveSession();
  if (!snapshot || !clipboard.value) return;
  const taskId = createTransferTask('copy', `${clipboard.value.items.length} 项`, snapshot.currentPath);
  try {
    await invoke('sftp_paste', { connectionId: snapshot.connectionId, items: clipboard.value.items, targetPath: snapshot.currentPath, mode: clipboard.value.mode, transferId: taskId });
    actionMessage.value = '粘贴完成。';
    if (clipboard.value.mode === 'cut') clearClipboard();
    await loadDir(snapshot.currentPath, snapshot.hostId);
  } catch {
    markTransferFailed(taskId);
    actionMessage.value = '粘贴失败，后端复制/剪切命令待完成或远程权限不足。';
  }
}

function openCreateDialog(kind: CreateEntryKind) {
  createDialog.visible = true;
  createDialog.kind = kind;
  createDialog.name = '';
  createDialog.error = '';
}

function closeCreateDialog() {
  createDialog.visible = false;
  createDialog.error = '';
}

async function confirmCreateEntry() {
  const normalizedName = normalizeRemoteName(createDialog.name);
  if (!normalizedName) {
    createDialog.error = '名称不能为空，且不能包含 / 或 \\';
    return;
  }

  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const path = joinRemotePath(snapshot.currentPath, normalizedName);
  const command = createDialog.kind === 'dir' ? 'sftp_mkdir' : 'sftp_create_file';
  await runSftpAction({ snapshot, loadingText: createDialog.kind === 'dir' ? '正在创建目录...' : '正在创建文件...', successText: '创建完成。', failureText: '创建失败，请检查名称或远程目录权限。', action: () => invoke(command, { connectionId: snapshot.connectionId, path }) });
  closeCreateDialog();
}

async function uploadFile() {
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const localPath = await open({ multiple: true, directory: false });
  if (!localPath) return;
  const paths = Array.isArray(localPath) ? localPath : [localPath];
  for (const path of paths) {
    await uploadOneFile(snapshot, path);
  }
}

async function uploadDirectory() {
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const localPath = await open({ directory: true, multiple: false });
  if (!localPath || Array.isArray(localPath)) return;
  await uploadOneDirectory(snapshot, localPath);
}

async function uploadOneFile(snapshot: { hostId: string; connectionId: string; currentPath: string }, localPath: string) {
  const fileName = getLocalFileName(localPath);
  if (!fileName) return;
  const remotePath = joinRemotePath(snapshot.currentPath, fileName);
  const transferId = createTransferTask('upload', fileName, snapshot.currentPath);
  await runSftpAction({ snapshot, loadingText: '正在上传...', successText: '上传完成。', failureText: '上传失败，请检查本地文件或远程目录权限。', transferId, action: () => invoke('sftp_upload_file', { connectionId: snapshot.connectionId, localPath, remotePath, transferId }) });
}

async function uploadOneDirectory(snapshot: { hostId: string; connectionId: string; currentPath: string }, localPath: string) {
  const dirName = getLocalFileName(localPath);
  if (!dirName) return;
  const remotePath = joinRemotePath(snapshot.currentPath, dirName);
  const transferId = createTransferTask('upload', dirName, snapshot.currentPath);
  await runSftpAction({ snapshot, loadingText: '正在上传文件夹...', successText: '文件夹上传完成。', failureText: '文件夹上传失败，请检查本地目录或远程目录权限。', transferId, action: () => invoke('sftp_upload_dir', { connectionId: snapshot.connectionId, localPath, remotePath, transferId }) });
}

async function downloadSelected() {
  const items = selectedItems.value;
  if (!items.length) return;

  if (items.length === 1 && !items[0].isDir) {
    await downloadOneFile(items[0]);
    return;
  }

  const targetDir = await open({ directory: true, multiple: false });
  if (!targetDir || Array.isArray(targetDir)) return;

  for (const item of items) {
    if (item.isDir) {
      await downloadOneDirectory(item, targetDir);
    } else {
      await downloadOneFile(item, joinLocalPath(targetDir, item.name));
    }
  }
}

async function downloadOneFile(item: RemoteFileItem, selectedLocalPath?: string) {
  if (item.isDir) return;
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const localPath = selectedLocalPath || await save({ defaultPath: item.name });
  if (!localPath) return;
  const transferId = createTransferTask('download', item.name, localPath, item.size);
  await runSftpAction({ snapshot, loadingText: '正在下载...', successText: '下载完成。', failureText: '下载失败，请检查远程文件或本地保存路径。', transferId, action: () => invoke('sftp_download_file', { connectionId: snapshot.connectionId, remotePath: item.path, localPath, transferId }) });
}

async function downloadOneDirectory(item: RemoteFileItem, localDir: string) {
  if (!item.isDir) return;
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const transferId = createTransferTask('download', item.name, localDir);
  await runSftpAction({ snapshot, loadingText: '正在下载文件夹...', successText: '文件夹下载完成。', failureText: '文件夹下载失败，请检查远程目录或本地保存路径。', transferId, action: () => invoke('sftp_download_dir', { connectionId: snapshot.connectionId, remotePath: item.path, localDir, transferId }) });
}

async function renameItem() {
  const item = selectedItems.value[0];
  if (!item) return;
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const newName = window.prompt('请输入新名称', item.name);
  const normalizedName = normalizeRemoteName(newName);
  if (!normalizedName || normalizedName === item.name) return;
  await runSftpAction({ snapshot, loadingText: '正在重命名...', successText: '重命名完成。', failureText: '重命名失败，请检查目标名称或目录权限。', action: () => invoke('sftp_rename', { connectionId: snapshot.connectionId, oldPath: item.path, newPath: joinRemotePath(snapshot.currentPath, normalizedName) }) });
}

function openDeleteDialog() {
  if (!selectedItems.value.length) return;
  deleteDialog.visible = true;
}

function closeDeleteDialog() {
  deleteDialog.visible = false;
}

async function confirmDeleteSelected() {
  const snapshot = await ensureActiveSession();
  if (!snapshot) return;
  const items = selectedItems.value.map((item) => ({ path: item.path, isDir: item.isDir }));
  const transferId = createTransferTask('delete', `${items.length} 项`, snapshot.currentPath);
  try {
    await invoke('sftp_delete_many', { connectionId: snapshot.connectionId, items, transferId });
    actionMessage.value = '删除完成。';
    clearSelection();
    await loadDir(snapshot.currentPath, snapshot.hostId);
  } catch {
    markTransferFailed(transferId);
    actionMessage.value = '删除失败，请检查目录是否为空或远程权限。';
  } finally {
    closeDeleteDialog();
  }
}

async function openPropertiesDialog(mode: 'properties' | 'chmod') {
  const item = selectedItems.value[0];
  const snapshot = await ensureActiveSession();
  if (!item || !snapshot) return;
  propertiesDialog.visible = true;
  propertiesDialog.item = item;
  propertiesDialog.stat = null;
  propertiesDialog.mode = mode;
  propertiesDialog.loading = true;
  propertiesDialog.error = '';
  chmodDialog.mode = item.isDir ? '755' : '644';

  try {
    const stat = await invoke<RemoteFileStat>('sftp_stat', { connectionId: snapshot.connectionId, path: item.path });
    if (!propertiesDialog.visible || propertiesDialog.item?.path !== item.path) return;
    propertiesDialog.stat = stat;
    if (stat.permissions && stat.permissions !== '-') {
      chmodDialog.mode = stat.permissions.slice(-4).replace(/^0+/, '') || stat.permissions;
    }
  } catch {
    if (!propertiesDialog.visible || propertiesDialog.item?.path !== item.path) return;
    propertiesDialog.error = '属性读取失败，请检查远程权限。';
  } finally {
    if (propertiesDialog.item?.path === item.path) {
      propertiesDialog.loading = false;
    }
  }
}

function closePropertiesDialog() {
  propertiesDialog.visible = false;
  propertiesDialog.item = null;
  propertiesDialog.stat = null;
  propertiesDialog.loading = false;
  propertiesDialog.error = '';
}

async function savePermissions() {
  const item = propertiesDialog.item;
  const snapshot = await ensureActiveSession();
  if (!item || !snapshot) return;
  if (!/^[0-7]{3,4}$/.test(chmodDialog.mode)) {
    actionMessage.value = '权限格式不正确，请输入 755 这类八进制权限。';
    return;
  }
  await runSftpAction({ snapshot, loadingText: '正在保存权限...', successText: '权限已保存。', failureText: '权限保存失败，请检查远程权限。', action: () => invoke('sftp_chmod', { connectionId: snapshot.connectionId, path: item.path, mode: chmodDialog.mode }) });
  closePropertiesDialog();
}

async function ensureActiveSession() {
  let snapshot = getActiveSessionSnapshot();
  if (!snapshot) {
    await syncSftpWithWorkspaceHost();
    snapshot = getActiveSessionSnapshot();
  }
  if (!snapshot) actionMessage.value = activeHost.value ? 'SFTP 尚未连接。' : '请先连接 SSH 主机。';
  return snapshot;
}

function getActiveSessionSnapshot() {
  const host = activeHost.value;
  const session = currentSession.value;
  if (!host || !session?.connectionId) return undefined;
  return { hostId: host.id, connectionId: session.connectionId, currentPath: session.currentPath };
}

async function runSftpAction(options: { snapshot: { hostId: string; connectionId: string; currentPath: string }; loadingText: string; successText: string; failureText: string; transferId?: string; action: () => Promise<unknown> }) {
  actionLoading.value = true;
  actionStatusText.value = options.loadingText;
  actionMessage.value = '';

  try {
    await options.action();
    const latest = sessionsByHostId.value[options.snapshot.hostId];
    if (!latest || latest.connectionId !== options.snapshot.connectionId) return;
    await loadDir(options.snapshot.currentPath, options.snapshot.hostId);
    if (activeHost.value?.id !== options.snapshot.hostId) return;
    actionMessage.value = options.successText;
  } catch {
    if (options.transferId) markTransferFailed(options.transferId);
    const latest = sessionsByHostId.value[options.snapshot.hostId];
    if (!latest || latest.connectionId !== options.snapshot.connectionId) return;
    if (activeHost.value?.id !== options.snapshot.hostId) return;
    actionMessage.value = options.failureText;
  } finally {
    actionLoading.value = false;
    actionStatusText.value = '';
  }
}

function createTransferTask(type: TransferType, name: string, targetPath: string, totalBytes = 0) {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  transferTasks.value = [{ id, type, name, targetPath, transferredBytes: 0, totalBytes, percent: 0, status: 'running', createdAt: Date.now() }, ...transferTasks.value].slice(0, 16);
  return id;
}

function markTransferFailed(transferId: string) {
  transferTasks.value = transferTasks.value.map((task) => task.id === transferId ? { ...task, status: 'failed' } : task);
}

function pauseAllTransfers() {
  transferTasks.value = transferTasks.value.map((task) => task.status === 'running' ? { ...task, status: 'cancelled' } : task);
}

function clearFinishedTransfers() {
  transferTasks.value = transferTasks.value.filter((task) => task.status === 'running' || task.status === 'queued');
}

function retryTransfer(task: TransferTask) {
  transferTasks.value = transferTasks.value.map((item) => item.id === task.id ? { ...item, status: 'queued', percent: 0, transferredBytes: 0 } : item);
}

function cancelTransfer(task: TransferTask) {
  transferTasks.value = transferTasks.value.map((item) => item.id === task.id ? { ...item, status: 'cancelled' } : item);
}

function transferStatusText(status: TransferStatus) {
  if (status === 'success') return '已完成';
  if (status === 'failed') return '失败';
  if (status === 'cancelled') return '已取消';
  if (status === 'queued') return '等待中';
  return '传输中';
}

function transferTypeText(type: TransferType) {
  if (type === 'upload') return '上传';
  if (type === 'download') return '下载';
  if (type === 'copy') return '复制';
  return '删除';
}

function showDropOverlay() {
  if (canRunConnectedAction.value) isDropActive.value = true;
}

function hideDropOverlay(event: DragEvent) {
  if (event.currentTarget === event.target) isDropActive.value = false;
}

function handleDropUpload() {
  isDropActive.value = false;
  actionMessage.value = '拖拽上传将在后续提交接入本地文件路径解析。';
}

function joinRemotePath(basePath: string, name: string) {
  if (basePath === '/') return `/${name}`;
  return `${basePath.replace(/\/+$/, '')}/${name}`;
}

function joinLocalPath(basePath: string, name: string) {
  return `${basePath.replace(/[\\/]+$/, '')}/${name}`;
}

function normalizeRemoteName(value: string | null) {
  const name = value?.trim() || '';
  if (!name || name === '.' || name === '..' || name.includes('/') || name.includes('\\')) return '';
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

function formatModifiedAt(value: number | undefined) {
  if (!value) return '-';
  return new Date(value * 1000).toLocaleString();
}

function formatOwner(stat: RemoteFileStat) {
  const uid = stat.uid === undefined ? '-' : stat.uid;
  const gid = stat.gid === undefined ? '-' : stat.gid;
  return `${uid}:${gid}`;
}

function formatSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${(size / 1024 / 1024 / 1024).toFixed(1)} GB`;
}
</script>

<style scoped>
.sftp-layout { display: flex; flex: 1; min-width: 0; flex-direction: column; padding: 18px; gap: 16px; }
.toolbar { display: flex; align-items: center; justify-content: space-between; min-height: 64px; border: 1px solid var(--ls-border); border-radius: 16px; background: var(--ls-panel); padding: 14px 18px; }
.toolbar h2 { margin: 0; font-size: 20px; }
.toolbar p { margin: 6px 0 0; color: var(--ls-text-muted); font-size: 13px; }
.status { border-radius: 999px; background: var(--ls-panel-strong); color: var(--ls-text-muted); padding: 6px 12px; font-size: 13px; }
.status--online { background: color-mix(in srgb, var(--ls-success) 14%, var(--ls-panel)); color: var(--ls-success); }
.content-grid { display: grid; min-height: 0; flex: 1; }
.browser-card { position: relative; display: flex; min-width: 0; min-height: 0; flex-direction: column; overflow: hidden; border: 1px solid var(--ls-border); border-radius: 12px; background: var(--ls-panel); }
.status-strip { display: flex; align-items: center; justify-content: space-between; gap: 8px; border-bottom: 1px solid var(--ls-border); background: var(--ls-panel-soft); color: var(--ls-text-muted); padding: 8px 12px; font-size: 12px; }
.status-strip--online { color: var(--ls-success); }
.clipboard-chip { border: 1px solid color-mix(in srgb, var(--ls-primary) 35%, var(--ls-border)); border-radius: 8px; background: var(--ls-primary-soft); color: var(--ls-primary); padding: 4px 8px; cursor: pointer; }
.sftp-toolbar-row { display: grid; align-items: center; grid-template-columns: minmax(180px, 420px) auto minmax(0, 1fr); gap: 8px; border-bottom: 1px solid var(--ls-border); background: linear-gradient(180deg, var(--ls-panel-strong), var(--ls-panel-soft)); padding: 6px 8px; }
.sftp-path-main { min-width: 0; width: 100%; max-width: 420px; height: 28px; overflow: hidden; border: 1px solid var(--ls-border); border-radius: 8px; background: var(--ls-panel); color: var(--ls-text); box-shadow: inset 0 1px 2px rgba(16, 24, 40, 0.08); font-size: 12px; line-height: 26px; padding: 0 10px; text-overflow: ellipsis; white-space: nowrap; }
.sftp-icon-actions { display: inline-flex; align-items: center; gap: 5px; }
.sftp-icon-button { display: inline-grid; width: 28px; height: 28px; place-items: center; overflow: hidden; border: 1px solid var(--ls-border-strong); border-radius: 8px; background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-strong)); color: var(--ls-text); box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.24), 0 1px 1px rgba(16, 24, 40, 0.06); cursor: pointer; font-family: "Segoe UI Symbol", "Arial Unicode MS", Arial, sans-serif; font-size: 15px; font-weight: 700; line-height: 1; padding: 0; text-align: center; white-space: nowrap; }
.sftp-icon-button:hover:not(:disabled) { border-color: var(--ls-primary); color: var(--ls-primary); }
.sftp-icon-button--danger:hover:not(:disabled) { border-color: var(--ls-danger); color: var(--ls-danger); }
.sftp-icon-button:disabled { cursor: not-allowed; opacity: 0.48; }
.action-message { min-width: 0; max-width: 220px; overflow: hidden; color: var(--ls-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.error-box { margin: 10px 10px 0; border: 1px solid color-mix(in srgb, var(--ls-danger) 38%, var(--ls-border)); border-radius: 10px; background: color-mix(in srgb, var(--ls-danger) 10%, var(--ls-panel)); color: var(--ls-danger); padding: 10px 12px; font-size: 13px; }
.table-wrap { position: relative; min-height: 0; flex: 1; overflow: auto; padding: 10px; }
.file-table { width: 100%; border-collapse: collapse; table-layout: fixed; }
.file-table th, .file-table td { border-bottom: 1px solid var(--ls-border); padding: 8px 10px; text-align: left; }
.file-table th { color: var(--ls-text-muted); font-size: 12px; font-weight: 600; }
.check-column { width: 38px; text-align: center !important; }
.file-table th:nth-child(3), .file-table td:nth-child(3) { width: 96px; }
.file-table th:nth-child(4), .file-table td:nth-child(4) { width: 120px; }
.file-table td { color: var(--ls-text); font-size: 13px; }
.file-table tbody tr { cursor: pointer; }
.file-table tbody tr:hover { background: var(--ls-panel-soft); }
.file-row--selected, .file-row--selected:hover { background: var(--ls-primary-soft); }
.file-row--anchor { box-shadow: inset 3px 0 0 var(--ls-primary); }
.name-cell { display: flex; min-width: 0; align-items: center; gap: 8px; }
.name-cell span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-icon { flex: 0 0 auto; }
.empty-cell { height: 180px; color: var(--ls-text-muted); text-align: center; }
.drop-overlay { position: absolute; inset: 48px 24px 72px; z-index: 3; display: grid; place-items: center; border: 1px dashed var(--ls-primary); border-radius: 12px; background: color-mix(in srgb, var(--ls-primary-soft) 70%, transparent); color: var(--ls-primary); pointer-events: none; }
.drop-overlay div { display: grid; gap: 8px; place-items: center; font-size: 18px; font-weight: 700; }
.drop-overlay strong { font-size: 40px; }
.selection-bar { display: flex; align-items: center; justify-content: space-between; min-height: 30px; border-top: 1px solid var(--ls-border); color: var(--ls-text-muted); padding: 0 12px; font-size: 12px; }
.context-menu { position: fixed; z-index: 30; min-width: 168px; margin: 0; border: 1px solid var(--ls-border); border-radius: 10px; background: var(--ls-panel); box-shadow: var(--ls-shadow-md); list-style: none; padding: 6px; }
.context-menu button { width: 100%; height: 28px; border-radius: 7px; background: transparent; color: var(--ls-text); text-align: left; padding: 0 10px; cursor: pointer; }
.context-menu button:hover:not(:disabled) { background: var(--ls-primary-soft); color: var(--ls-primary); }
.context-menu button:disabled { color: var(--ls-text-muted); cursor: not-allowed; opacity: 0.55; }
.context-menu button.danger { color: var(--ls-danger); }
.context-menu__separator { height: 1px; margin: 4px 0; background: var(--ls-border); }
.transfer-panel { border-top: 1px solid var(--ls-border); background: var(--ls-panel-soft); }
.transfer-panel__head { display: flex; align-items: center; justify-content: space-between; min-height: 34px; color: var(--ls-text); padding: 0 12px; font-size: 12px; }
.transfer-title-button { display: inline-flex; align-items: center; gap: 8px; background: transparent; color: var(--ls-text); cursor: pointer; }
.transfer-actions { display: inline-flex; gap: 8px; }
.transfer-actions button, .transfer-row-actions button { border: 1px solid var(--ls-border); border-radius: 7px; background: var(--ls-panel); color: var(--ls-text); cursor: pointer; padding: 3px 8px; }
.transfer-actions button:disabled { cursor: not-allowed; opacity: 0.45; }
.transfer-list--table { display: grid; max-height: 180px; overflow: auto; padding: 0 10px 10px; }
.transfer-row { display: grid; align-items: center; grid-template-columns: 52px minmax(120px, 1fr) minmax(160px, 1fr) 88px 132px 72px 88px; gap: 8px; min-height: 32px; border-bottom: 1px solid var(--ls-border); color: var(--ls-text-muted); font-size: 12px; }
.transfer-row--head { color: var(--ls-text-muted); font-weight: 700; }
.transfer-row strong { overflow: hidden; color: var(--ls-text); text-overflow: ellipsis; white-space: nowrap; }
.transfer-progress-cell { display: grid; align-items: center; grid-template-columns: minmax(0, 1fr) 42px; gap: 6px; }
.transfer-progress-cell i { display: block; height: 6px; border-radius: 999px; background: var(--ls-primary); }
.transfer-progress__bar--success { background: var(--ls-success) !important; }
.transfer-progress__bar--failed { background: var(--ls-danger) !important; }
.transfer-progress__bar--cancelled { background: var(--ls-border-strong) !important; }
.transfer-status--success { color: var(--ls-success); }
.transfer-status--failed { color: var(--ls-danger); }
.transfer-status--running { color: var(--ls-primary); }
.transfer-row-actions { display: inline-flex; gap: 4px; }
.sftp-modal-mask { position: fixed; inset: 0; z-index: 40; display: grid; place-items: center; background: rgba(15, 23, 42, 0.28); backdrop-filter: blur(3px); }
.sftp-modal { display: grid; gap: 14px; border: 1px solid var(--ls-border); border-radius: 12px; background: var(--ls-panel); box-shadow: var(--ls-shadow-lg); color: var(--ls-text); padding: 14px; }
.sftp-modal--small { width: min(420px, calc(100vw - 48px)); }
.sftp-modal--properties { width: min(520px, calc(100vw - 48px)); }
.sftp-modal header, .sftp-modal footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sftp-modal header button { background: transparent; color: var(--ls-text-muted); cursor: pointer; font-size: 18px; }
.sftp-modal footer { justify-content: flex-end; }
.sftp-modal footer button { height: 32px; border: 1px solid var(--ls-border); border-radius: 8px; background: var(--ls-panel-soft); color: var(--ls-text); cursor: pointer; padding: 0 16px; }
.sftp-modal footer button.primary { border-color: var(--ls-primary); background: var(--ls-primary); color: #fff; }
.sftp-modal footer button.danger { border-color: var(--ls-danger); background: var(--ls-danger); color: #fff; }
.segmented-control { display: inline-flex; width: max-content; overflow: hidden; border: 1px solid var(--ls-border); border-radius: 8px; }
.segmented-control button { height: 30px; border-right: 1px solid var(--ls-border); background: var(--ls-panel-soft); color: var(--ls-text); cursor: pointer; padding: 0 12px; }
.segmented-control button:last-child { border-right: 0; }
.segmented-control button.active { background: var(--ls-primary-soft); color: var(--ls-primary); }
.modal-field { display: grid; gap: 6px; color: var(--ls-text-muted); font-size: 12px; }
.modal-field input { height: 34px; padding: 0 10px; }
.modal-field--inline { grid-template-columns: 72px 120px minmax(0, 1fr); align-items: center; }
.modal-error { margin: 0; color: var(--ls-danger); font-size: 12px; }
.delete-warning { display: grid; grid-template-columns: 42px minmax(0, 1fr); gap: 10px; }
.delete-warning b { display: grid; width: 36px; height: 36px; place-items: center; border: 2px solid var(--ls-danger); border-radius: 999px; color: var(--ls-danger); font-size: 24px; }
.delete-warning h4 { margin: 0 0 6px; }
.delete-warning p, .compact-hint { margin: 0; color: var(--ls-text-muted); font-size: 12px; }
.properties-list { display: grid; gap: 10px; margin: 0; }
.properties-list div { display: grid; grid-template-columns: 82px minmax(0, 1fr); gap: 12px; }
.properties-list dt { color: var(--ls-text-muted); }
.properties-list dd { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chmod-box { display: grid; gap: 10px; border: 1px solid var(--ls-border); border-radius: 10px; background: var(--ls-panel-soft); padding: 12px; }
.chmod-box h4 { margin: 0; color: var(--ls-primary); }
@media (max-width: 1000px) { .sftp-toolbar-row { grid-template-columns: minmax(140px, 1fr); } .sftp-icon-actions { flex-wrap: wrap; } .transfer-row { grid-template-columns: 48px minmax(120px, 1fr) 100px 90px; } .transfer-row span:nth-child(3), .transfer-row span:nth-child(6), .transfer-row span:nth-child(7) { display: none; } }
</style>
