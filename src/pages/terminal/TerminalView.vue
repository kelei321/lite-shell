<template>
  <section class="terminal-layout" :class="themeClass">
    <header class="toolbar">
      <div>
        <h2>SSH 终端</h2>
        <p>每个新页签先进入快速连接；连接管理器以弹窗方式管理主机。</p>
      </div>
      <div class="toolbar-actions">
        <button class="theme-toggle" type="button" @click="toggleTheme">
          <span>{{ isDarkTheme ? '🌙' : '☀️' }}</span>
          {{ isDarkTheme ? '暗色' : '浅色' }}
        </button>
        <button class="ghost-button" type="button" @click="openConnectionManager">连接管理器</button>
        <button class="primary-button" type="button" @click="createQuickTab">新标签页</button>
      </div>
    </header>

    <div class="content-grid">
      <section class="workspace-card">
        <div class="tabs-bar">
          <button class="tab-tool" title="连接管理器" type="button" @click="openConnectionManager">📁</button>
          <button
            v-for="tab in tabs"
            :key="tab.id"
            class="tab-item"
            :class="{ 'tab-item--active': tab.id === activeTabId }"
            type="button"
            @click="activateTab(tab.id)"
          >
            <span>{{ tab.title }}</span>
            <span class="tab-dot" :class="{ 'tab-dot--online': tab.sessionId }"></span>
            <span class="tab-close" @click.stop="closeTab(tab.id)">×</span>
          </button>
          <button class="tab-add" title="新标签页" type="button" @click="createQuickTab">＋</button>
        </div>

        <div class="terminal-card">
          <template v-for="tab in tabs" :key="tab.id">
            <div
              v-if="tab.kind === 'terminal'"
              v-show="tab.id === activeTabId"
              :ref="(el) => setTerminalHost(tab.id, el)"
              class="terminal-host"
            ></div>
            <div v-else v-show="tab.id === activeTabId" class="quick-connect-page">
              <section class="quick-connect-main">
                <div class="quick-connect-head card-head">
                  <div>
                    <h3>快速连接</h3>
                    <p>选择常用主机或填写连接信息；密码仅用于本次连接，不会保存。</p>
                  </div>
                  <button class="ghost-button" type="button" @click="openConnectionManager">打开连接管理器</button>
                </div>

                <div class="search-control">
                  <span>⌕</span>
                  <input v-model.trim="hostSearch" autocomplete="off" placeholder="搜索主机（名称 / 主机 / IP）" />
                </div>

                <div class="quick-host-list">
                  <div class="quick-host-list__title">
                    <strong>最近连接</strong>
                    <span>{{ hostStore.recentHosts.length ? '双击选择主机' : '连接后会显示最近主机' }}</span>
                  </div>
                  <button
                    v-for="host in quickHosts"
                    :key="host.id"
                    class="quick-host-row"
                    type="button"
                    @click="selectHost(host)"
                    @dblclick="selectHost(host)"
                  >
                    <span class="host-icon">▣</span>
                    <span class="host-name">{{ host.name }}</span>
                    <em>{{ host.host }}</em>
                    <strong>{{ host.username }}</strong>
                    <b>{{ host.port }}</b>
                  </button>
                  <p v-if="quickHosts.length === 0" class="empty-tip">暂无主机，点击“打开连接管理器”新建连接。</p>
                </div>
              </section>

              <form class="quick-connect-form" @submit.prevent="saveAndConnect">
                <div class="panel-title-row card-head">
                  <div>
                    <h3>连接到主机</h3>
                    <p>填写主机连接信息</p>
                  </div>
                  <button class="tiny-button" type="button" @click="resetForm">清空</button>
                </div>
                <label><span>名称</span><input v-model.trim="form.name" autocomplete="off" placeholder="例如：生产服务器" /></label>
                <label><span>分组</span><input v-model.trim="form.group" autocomplete="off" list="host-group-options" placeholder="默认分组" /></label>
                <datalist id="host-group-options">
                  <option v-for="group in hostStore.groups" :key="group" :value="group"></option>
                </datalist>
                <label><span>主机</span><input v-model.trim="form.host" autocomplete="off" placeholder="例如：192.168.1.100 或 example.com" /></label>
                <label><span>端口</span><input v-model.number="form.port" min="1" max="65535" type="number" /></label>
                <label><span>用户名</span><input v-model.trim="form.username" autocomplete="username" placeholder="例如：root" /></label>
                <label><span>密码</span><input v-model="form.password" autocomplete="current-password" placeholder="当前连接使用，不会保存" type="password" /></label>
                <p class="security-tip">密码只进入当前 SSH 连接流程，不写入 localStorage。</p>
                <div class="action-row">
                  <button class="ghost-button" :disabled="!canSave" type="button" @click="saveHost">保存</button>
                  <button class="primary-button" :disabled="!canConnect || connecting" type="submit">
                    {{ connecting ? '连接中...' : '连接' }}
                  </button>
                </div>
              </form>
            </div>
          </template>
        </div>
      </section>
    </div>

    <div v-if="isConnectionManagerOpen" class="modal-mask" @click.self="closeConnectionManager">
      <section class="connection-manager-dialog" role="dialog" aria-modal="true" aria-label="连接管理器">
        <header class="dialog-head">
          <div>
            <h3>连接管理器</h3>
            <p>分组管理连接，选择主机后在快速连接页输入密码。</p>
          </div>
          <button class="dialog-close" type="button" @click="closeConnectionManager">×</button>
        </header>

        <div class="manager-toolbar manager-toolbar--top">
          <button class="primary-button" type="button" @click="resetForm">＋ 新建连接</button>
          <button class="ghost-button" type="button" @click="resetForm">＋ 新建分组</button>
          <div class="search-control">
            <span>⌕</span>
            <input v-model.trim="hostSearch" autocomplete="off" placeholder="搜索连接..." />
          </div>
          <select v-model="selectedGroup">
            <option value="">全部分组</option>
            <option v-for="group in hostStore.groups" :key="group" :value="group">{{ group }}</option>
          </select>
        </div>

        <div class="dialog-body">
          <aside class="manager-list">
            <section v-if="hostStore.recentHosts.length" class="manager-section">
              <h4>快速连接</h4>
              <button
                v-for="host in hostStore.recentHosts"
                :key="host.id"
                class="manager-host manager-host--recent"
                type="button"
                @click="selectHost(host)"
                @dblclick="selectHostAndClose(host)"
              >
                <span>{{ host.name }}</span>
                <em>{{ host.host }}</em>
                <strong>{{ host.username }}</strong>
              </button>
            </section>

            <section class="manager-section manager-section--tree">
              <div class="manager-section__head">
                <h4>连接列表</h4>
                <span>{{ filteredHosts.length }} 个连接</span>
              </div>

              <p v-if="hostStore.sortedHosts.length === 0" class="empty-tip">暂无连接。</p>
              <p v-else-if="filteredHosts.length === 0" class="empty-tip">没有匹配的连接。</p>

              <div v-else class="connection-tree">
                <section v-for="group in groupedFilteredHosts" :key="group.name" class="tree-group">
                  <div class="tree-group__head">
                    <span>▾ 📁 {{ group.name }}</span>
                    <em>{{ group.hosts.length }}</em>
                  </div>
                  <button
                    v-for="host in group.hosts"
                    :key="host.id"
                    class="manager-host tree-host"
                    :class="{ 'manager-host--active': form.id === host.id }"
                    type="button"
                    @click="selectHost(host)"
                    @dblclick="selectHostAndClose(host)"
                  >
                    <span>{{ host.name }}</span>
                    <em>{{ host.host }}:{{ host.port }}</em>
                    <strong>{{ host.username }}</strong>
                  </button>
                </section>
              </div>
            </section>
          </aside>

          <form class="manager-editor" @submit.prevent="saveAndConnect">
            <div class="panel-title-row card-head">
              <div>
                <h3>连接编辑</h3>
                <p>保存元数据，密码不落盘</p>
              </div>
              <button class="tiny-button" type="button" @click="resetForm">重置</button>
            </div>
            <label><span>名称</span><input v-model.trim="form.name" autocomplete="off" placeholder="生产服务器" /></label>
            <label><span>分组</span><input v-model.trim="form.group" autocomplete="off" list="host-group-options" placeholder="默认分组" /></label>
            <label><span>主机</span><input v-model.trim="form.host" autocomplete="off" placeholder="127.0.0.1" /></label>
            <label><span>端口</span><input v-model.number="form.port" min="1" max="65535" type="number" /></label>
            <label><span>用户名</span><input v-model.trim="form.username" autocomplete="username" placeholder="root" /></label>
            <label><span>密码</span><input v-model="form.password" autocomplete="current-password" type="password" /></label>
            <label><span>备注</span><textarea v-model.trim="form.remark" placeholder="可选：添加备注信息..."></textarea></label>
            <p class="security-tip">密码仅用于当前连接，不会保存到本地或服务器。</p>
            <div class="action-row">
              <button class="ghost-button" :disabled="!canSave" type="button" @click="saveHost">保存</button>
              <button class="primary-button" :disabled="!canConnect || connecting" type="submit">
                {{ connecting ? '连接中...' : '连接' }}
              </button>
            </div>
          </form>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, markRaw, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';

import { type HostProfile, useHostStore } from '@/stores/hosts';
import { useWorkspaceStore } from '@/stores/workspace';

import '@xterm/xterm/css/xterm.css';

interface SshDataPayload {
  id: string;
  data: string;
}

interface TerminalTab {
  id: string;
  kind: 'quick' | 'terminal';
  hostId: string;
  title: string;
  host: string;
  port: number;
  username: string;
  sessionId: string;
  connecting: boolean;
  statusText: string;
  terminal?: Terminal;
  fitAddon?: FitAddon;
  lastCols?: number;
  lastRows?: number;
}

type TerminalTheme = 'light' | 'dark';

const THEME_STORAGE_KEY = 'lite-shell.terminal.theme';
const hostStore = useHostStore();
const workspaceStore = useWorkspaceStore();

const form = reactive({ id: '', name: '', group: '默认分组', host: '127.0.0.1', port: 22, username: 'root', password: '', remark: '' });
const tabs = ref<TerminalTab[]>([]);
const activeTabId = ref('');
const connecting = ref(false);
const hostSearch = ref('');
const selectedGroup = ref('');
const isConnectionManagerOpen = ref(false);
const theme = ref<TerminalTheme>('light');
const terminalHosts = new Map<string, HTMLDivElement>();

const activeTab = computed(() => tabs.value.find((tab) => tab.id === activeTabId.value));
const canSave = computed(() => Boolean(form.host.trim() && form.username.trim()));
const canConnect = computed(() => Boolean(canSave.value && form.password));
const isDarkTheme = computed(() => theme.value === 'dark');
const themeClass = computed(() => `terminal-layout--${theme.value}`);
const quickHosts = computed(() => hostStore.recentHosts.length ? hostStore.recentHosts : hostStore.sortedHosts.slice(0, 9));
const filteredHosts = computed(() => {
  const keyword = hostSearch.value.trim().toLowerCase();

  return hostStore.sortedHosts.filter((host) => {
    const matchesGroup = !selectedGroup.value || host.group === selectedGroup.value;
    const haystack = [host.name, host.group, host.host, host.username, String(host.port)]
      .join(' ')
      .toLowerCase();
    return matchesGroup && (!keyword || haystack.includes(keyword));
  });
});
const groupedFilteredHosts = computed(() => {
  const groups = new Map<string, HostProfile[]>();

  for (const host of filteredHosts.value) {
    const groupName = host.group || '默认分组';
    const groupHosts = groups.get(groupName) || [];
    groupHosts.push(host);
    groups.set(groupName, groupHosts);
  }

  return [...groups.entries()].map(([name, hosts]) => ({ name, hosts }));
});

let unlisten: UnlistenFn | undefined;

onMounted(async () => {
  restoreThemePreference();
  createQuickTab();
  unlisten = await listen<SshDataPayload>('ssh:data', (event) => {
    const tab = tabs.value.find((item) => item.sessionId === event.payload.id);
    tab?.terminal?.write(event.payload.data);
  });
  window.addEventListener('resize', handleWindowResize);
});

onBeforeUnmount(() => {
  unlisten?.();
  window.removeEventListener('resize', handleWindowResize);
  for (const tab of tabs.value) {
    if (tab.sessionId) void invoke('ssh_close', { id: tab.sessionId });
    if (tab.hostId) workspaceStore.clearCredential(tab.hostId);
    tab.terminal?.dispose();
  }
  workspaceStore.clearAllCredentials();
});

function createQuickTab() {
  const tab: TerminalTab = {
    id: createId(),
    kind: 'quick',
    hostId: '',
    title: '新标签页',
    host: '',
    port: 22,
    username: '',
    sessionId: '',
    connecting: false,
    statusText: '快速连接',
  };

  tabs.value.push(tab);
  activeTabId.value = tab.id;
  resetForm();
  workspaceStore.clearActiveHost();
}

function openConnectionManager() {
  isConnectionManagerOpen.value = true;
}

function closeConnectionManager() {
  isConnectionManagerOpen.value = false;
}

function toggleTheme() {
  theme.value = isDarkTheme.value ? 'light' : 'dark';
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme.value);
  } catch {
    // Ignore localStorage quota or privacy-mode failures.
  }
}

function restoreThemePreference() {
  try {
    const value = localStorage.getItem(THEME_STORAGE_KEY);
    if (value === 'light' || value === 'dark') theme.value = value;
  } catch {
    // Ignore localStorage read failures.
  }
}

function resetForm() {
  form.id = '';
  form.name = '';
  form.group = selectedGroup.value || '默认分组';
  form.host = '127.0.0.1';
  form.port = 22;
  form.username = 'root';
  form.password = '';
  form.remark = '';
}

function selectHost(host: HostProfile) {
  form.id = host.id;
  form.name = host.name;
  form.group = host.group || '默认分组';
  form.host = host.host;
  form.port = host.port;
  form.username = host.username;
  form.password = '';
  form.remark = host.remark || '';
}

function selectHostAndClose(host: HostProfile) {
  selectHost(host);
  closeConnectionManager();
}

function saveHost() {
  if (!canSave.value) return undefined;
  const host = hostStore.upsertHost({ id: form.id || undefined, name: form.name, group: form.group, host: form.host, port: form.port, username: form.username, remark: form.remark });
  if (host) selectHost(host);
  return host;
}

async function saveAndConnect() {
  if (!canConnect.value || connecting.value) return;
  const password = form.password;
  const host = saveHost();
  if (!host) return;
  form.password = '';
  closeConnectionManager();
  await openTerminalTab(host, password);
}

async function openTerminalTab(host: HostProfile, password: string) {
  let tab = activeTab.value;
  if (!tab || tab.kind !== 'quick') {
    tab = {
      id: createId(),
      kind: 'terminal',
      hostId: host.id,
      title: host.name,
      host: host.host,
      port: host.port,
      username: host.username,
      sessionId: '',
      connecting: true,
      statusText: '连接中',
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
  } else {
    tab.kind = 'terminal';
    tab.hostId = host.id;
    tab.title = host.name;
    tab.host = host.host;
    tab.port = host.port;
    tab.username = host.username;
    tab.connecting = true;
    tab.statusText = '连接中';
  }

  connecting.value = true;
  await nextTick();
  ensureTerminal(tab);
  await connectTab(tab, host, password);
}

async function connectTab(tab: TerminalTab, host: HostProfile, password: string) {
  const terminal = tab.terminal;
  if (!terminal) return;
  terminal.clear();
  terminal.writeln(`Connecting to ${tab.username}@${tab.host}:${tab.port} ...`);

  try {
    fitVisibleTab(tab, { resizeRemote: false });
    const sessionId = await invoke<string>('ssh_connect', {
      payload: { host: tab.host, port: tab.port, username: tab.username, password, privateKeyPath: null, passphrase: null, cols: terminal.cols, rows: terminal.rows },
    });

    tab.sessionId = sessionId;
    tab.lastCols = terminal.cols;
    tab.lastRows = terminal.rows;
    tab.statusText = '已连接';
    hostStore.touchHost(host.id);
    workspaceStore.setActiveHost(host);
    workspaceStore.setCredential({
      hostId: host.id,
      host: host.host,
      port: host.port,
      username: host.username,
      password,
      source: 'ssh',
      createdAt: Date.now(),
    });
    terminal.writeln('Connected.');
    terminal.focus();
  } catch {
    tab.statusText = '连接失败';
    terminal.writeln('Connect failed.');
  } finally {
    tab.connecting = false;
    connecting.value = tabs.value.some((item) => item.connecting);
  }
}

function setTerminalHost(tabId: string, element: unknown) {
  if (!(element instanceof HTMLDivElement)) return;
  terminalHosts.set(tabId, element);
  const tab = tabs.value.find((item) => item.id === tabId);
  if (tab) ensureTerminal(tab);
}

function ensureTerminal(tab: TerminalTab) {
  if (tab.kind !== 'terminal' || tab.terminal) return;
  const hostElement = terminalHosts.get(tab.id);
  if (!hostElement) return;

  const terminal = new Terminal({ cursorBlink: true, convertEol: true, fontFamily: 'Consolas, "JetBrains Mono", "Noto Sans Mono CJK SC", monospace', fontSize: 14, scrollback: 6000, theme: { background: '#020617', foreground: '#e5e7eb' } });
  const fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(hostElement);
  terminal.writeln('LiteShell ready.');
  hostElement.addEventListener('pointerdown', () => terminal.focus());
  terminal.onData((data) => {
    if (!tab.sessionId) return;
    void invoke('ssh_write', { id: tab.sessionId, data });
  });

  tab.terminal = markRaw(terminal);
  tab.fitAddon = markRaw(fitAddon);
  if (tab.id === activeTabId.value) {
    fitVisibleTab(tab, { resizeRemote: false });
    terminal.focus();
  }
}

function activateTab(tabId: string) {
  activeTabId.value = tabId;
  const tab = tabs.value.find((item) => item.id === tabId);
  if (tab?.kind === 'terminal') {
    workspaceStore.setActiveHost({ id: tab.hostId, name: tab.title, host: tab.host, port: tab.port, username: tab.username });
  } else {
    workspaceStore.clearActiveHost();
  }
  scheduleActiveTabRefresh();
}

async function closeTab(tabId: string) {
  const index = tabs.value.findIndex((tab) => tab.id === tabId);
  if (index < 0) return;
  const tab = tabs.value[index];
  if (tab.sessionId) await invoke('ssh_close', { id: tab.sessionId });
  terminalHosts.delete(tab.id);
  tab.terminal?.dispose();
  tabs.value.splice(index, 1);

  if (tab.hostId) {
    const stillHasSameHostTab = tabs.value.some((item) => item.hostId === tab.hostId);
    if (!stillHasSameHostTab) {
      workspaceStore.clearCredential(tab.hostId);
    }
  }

  if (tabs.value.length === 0) {
    createQuickTab();
    return;
  }

  if (activeTabId.value === tabId) {
    const next = tabs.value[Math.max(0, index - 1)] || tabs.value[0];
    activeTabId.value = next.id;
    if (next.kind === 'terminal') workspaceStore.setActiveHost({ id: next.hostId, name: next.title, host: next.host, port: next.port, username: next.username });
    else workspaceStore.clearActiveHost(tab.hostId);
    scheduleActiveTabRefresh();
  }
}

function handleWindowResize() {
  scheduleActiveTabResize();
}

function scheduleActiveTabRefresh() {
  const tabId = activeTabId.value;
  void nextTick(() => requestAnimationFrame(() => {
    if (activeTabId.value !== tabId) return;
    const terminal = activeTab.value?.terminal;
    if (!terminal) return;
    terminal.refresh(0, Math.max(0, terminal.rows - 1));
    terminal.focus();
  }));
}

function scheduleActiveTabResize() {
  const tabId = activeTabId.value;
  void nextTick(() => requestAnimationFrame(() => {
    if (activeTabId.value !== tabId) return;
    const tab = activeTab.value;
    if (!tab) return;
    fitVisibleTab(tab, { resizeRemote: true });
    tab.terminal?.focus();
  }));
}

function fitVisibleTab(tab: TerminalTab, options: { resizeRemote: boolean }) {
  if (tab.kind !== 'terminal' || !tab.terminal || !tab.fitAddon) return;
  const hostElement = terminalHosts.get(tab.id);
  if (!hostElement) return;
  const rect = hostElement.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;
  tab.fitAddon.fit();
  const { cols, rows } = tab.terminal;
  if (!options.resizeRemote || !tab.sessionId || cols <= 0 || rows <= 0) return;
  if (tab.lastCols === cols && tab.lastRows === rows) return;
  tab.lastCols = cols;
  tab.lastRows = rows;
  void invoke('ssh_resize', { id: tab.sessionId, cols, rows });
}

function createId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
</script>

<style scoped>
.terminal-layout {
  --ls-bg: #f4f6f8;
  --ls-panel: #ffffff;
  --ls-panel-soft: #f8f9fa;
  --ls-panel-strong: #eef1f4;
  --ls-border: #d7dde4;
  --ls-border-strong: #bdc6d0;
  --ls-text: #1f2937;
  --ls-text-muted: #667085;
  --ls-primary: #0d6efd;
  --ls-primary-hover: #0b5ed7;
  --ls-primary-soft: #e7f1ff;
  --ls-success: #22c55e;
  --ls-shadow-sm: 0 1px 2px rgba(16, 24, 40, 0.08), inset 0 1px 0 rgba(255, 255, 255, 0.72);
  --ls-shadow-md: 0 10px 28px rgba(16, 24, 40, 0.14), inset 0 1px 0 rgba(255, 255, 255, 0.8);
  --ls-terminal-bg: #020617;
  --ls-terminal-text: #e5e7eb;

  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
  background: var(--ls-bg);
  color: var(--ls-text);
}

.terminal-layout--dark {
  --ls-bg: #0e1624;
  --ls-panel: #121c2b;
  --ls-panel-soft: #0f1726;
  --ls-panel-strong: #182235;
  --ls-border: #253349;
  --ls-border-strong: #33445e;
  --ls-text: #f1f5f9;
  --ls-text-muted: #98a6b8;
  --ls-primary: #2f81ff;
  --ls-primary-hover: #1f6feb;
  --ls-primary-soft: rgba(47, 129, 255, 0.16);
  --ls-success: #2ecc71;
  --ls-shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.34), inset 0 1px 0 rgba(255, 255, 255, 0.04);
  --ls-shadow-md: 0 18px 42px rgba(0, 0, 0, 0.34), inset 0 1px 0 rgba(255, 255, 255, 0.05);
}

.toolbar,
.workspace-card,
.quick-connect-main,
.quick-connect-form,
.manager-list,
.manager-editor,
.connection-manager-dialog {
  border: 1px solid var(--ls-border);
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-soft));
  box-shadow: var(--ls-shadow-sm);
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 62px;
  border-radius: 12px;
  padding: 12px 16px;
}

.toolbar h2,
.quick-connect-head h3,
.quick-connect-form h3,
.manager-editor h3,
.dialog-head h3,
.panel-title-row h3,
.manager-section h4 {
  margin: 0;
}

.toolbar h2 {
  font-size: 19px;
}

.toolbar p,
.quick-connect-head p,
.dialog-head p,
.card-head p {
  margin: 4px 0 0;
  color: var(--ls-text-muted);
  font-size: 12px;
}

.toolbar-actions,
.card-head,
.quick-connect-head,
.panel-title-row,
.quick-host-list__title,
.manager-section__head,
.dialog-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.toolbar-actions {
  flex-wrap: wrap;
}

.content-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-height: 0;
  flex: 1;
}

.workspace-card {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  border-radius: 12px;
}

.tabs-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 42px;
  border-bottom: 1px solid var(--ls-border);
  background: linear-gradient(180deg, var(--ls-panel-strong), var(--ls-panel-soft));
  padding: 6px 8px;
  overflow-x: auto;
}

.tab-tool,
.tab-add,
.tab-item,
.primary-button,
.ghost-button,
.tiny-button,
.theme-toggle,
.dialog-close,
.manager-toolbar select {
  border: 1px solid var(--ls-border-strong);
  border-radius: 8px;
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-strong));
  color: var(--ls-text);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.24), 0 1px 1px rgba(16, 24, 40, 0.06);
  cursor: pointer;
}

.tab-tool,
.tab-add,
.dialog-close {
  display: grid;
  width: 30px;
  height: 30px;
  flex: 0 0 auto;
  place-items: center;
}

.tab-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 9px 0 10px;
  white-space: nowrap;
}

.tab-item--active {
  border-color: var(--ls-primary);
  background: linear-gradient(180deg, var(--ls-primary-soft), var(--ls-panel));
  color: var(--ls-text);
}

.tab-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: var(--ls-border-strong);
}

.tab-dot--online {
  background: var(--ls-success);
}

.tab-close {
  color: var(--ls-text-muted);
  font-size: 15px;
  line-height: 1;
}

.tab-close:hover {
  color: var(--ls-text);
}

.terminal-card {
  position: relative;
  min-width: 0;
  min-height: 0;
  flex: 1;
  padding: 12px;
}

.terminal-host {
  width: 100%;
  height: 100%;
  overflow: hidden;
  border: 1px solid var(--ls-border-strong);
  border-radius: 10px;
  background: var(--ls-terminal-bg);
  color: var(--ls-terminal-text);
  box-shadow: inset 0 1px 8px rgba(0, 0, 0, 0.45);
}

.quick-connect-page {
  display: grid;
  height: 100%;
  grid-template-columns: minmax(0, 1fr) 340px;
  gap: 14px;
}

.quick-connect-main,
.quick-connect-form,
.manager-list,
.manager-editor {
  min-height: 0;
  border-radius: 12px;
  padding: 14px;
}

.quick-connect-form,
.manager-editor {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.search-control {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  margin-top: 12px;
  border: 1px solid var(--ls-border);
  border-radius: 8px;
  background: var(--ls-panel);
  box-shadow: inset 0 1px 2px rgba(16, 24, 40, 0.08);
  padding: 0 10px;
}

.search-control span {
  color: var(--ls-text-muted);
}

.search-control input {
  min-width: 0;
  flex: 1;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--ls-text);
}

.quick-host-list {
  display: grid;
  gap: 7px;
  margin-top: 14px;
}

.quick-host-list__title span,
.empty-tip,
.security-tip {
  color: var(--ls-text-muted);
  font-size: 12px;
}

.quick-host-row,
.manager-host {
  display: grid;
  align-items: center;
  grid-template-columns: 24px minmax(0, 1fr) 132px 74px 42px;
  gap: 8px;
  min-height: 36px;
  border: 1px solid var(--ls-border);
  border-radius: 8px;
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-soft));
  color: var(--ls-text);
  padding: 0 10px;
  text-align: left;
  box-shadow: var(--ls-shadow-sm);
  cursor: pointer;
}

.quick-host-row:hover,
.manager-host:hover,
.manager-host--active {
  border-color: var(--ls-primary);
  background: linear-gradient(180deg, var(--ls-primary-soft), var(--ls-panel));
}

.host-icon {
  color: var(--ls-primary);
}

.host-name,
.quick-host-row em,
.manager-host span,
.manager-host em,
.manager-host strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quick-host-row em,
.manager-host em,
.manager-host strong,
.quick-host-row strong {
  color: var(--ls-text-muted);
  font-size: 12px;
  font-style: normal;
  font-weight: 500;
}

.quick-host-row b {
  display: inline-grid;
  min-width: 32px;
  height: 22px;
  place-items: center;
  border: 1px solid var(--ls-border);
  border-radius: 6px;
  background: var(--ls-panel-soft);
  font-size: 12px;
  font-weight: 500;
}

.quick-connect-form label,
.manager-editor label {
  display: grid;
  gap: 6px;
}

.quick-connect-form span,
.manager-editor span {
  color: var(--ls-text);
  font-size: 12px;
  font-weight: 600;
}

.quick-connect-form input,
.manager-editor input,
.manager-editor textarea,
.manager-toolbar select {
  width: 100%;
  min-width: 0;
  border: 1px solid var(--ls-border);
  border-radius: 8px;
  outline: none;
  background: var(--ls-panel);
  color: var(--ls-text);
  box-shadow: inset 0 1px 2px rgba(16, 24, 40, 0.08);
  padding: 0 10px;
}

.quick-connect-form input,
.manager-editor input,
.manager-toolbar select {
  height: 34px;
}

.manager-editor textarea {
  min-height: 74px;
  padding: 9px 10px;
  resize: vertical;
}

.quick-connect-form input:focus,
.manager-editor input:focus,
.manager-editor textarea:focus,
.search-control:focus-within {
  border-color: var(--ls-primary);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--ls-primary) 16%, transparent), inset 0 1px 2px rgba(16, 24, 40, 0.08);
}

.security-tip {
  margin: 0;
  border: 1px solid color-mix(in srgb, var(--ls-primary) 35%, var(--ls-border));
  border-radius: 8px;
  background: var(--ls-primary-soft);
  padding: 9px 10px;
}

.action-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-top: auto;
}

.primary-button,
.ghost-button,
.theme-toggle {
  height: 34px;
  padding: 0 12px;
}

.theme-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.primary-button {
  border-color: var(--ls-primary);
  background: linear-gradient(180deg, color-mix(in srgb, var(--ls-primary) 88%, #ffffff 12%), var(--ls-primary-hover));
  color: #fff;
}

.primary-button:hover {
  background: var(--ls-primary-hover);
}

.ghost-button:hover,
.tiny-button:hover,
.theme-toggle:hover,
.tab-tool:hover,
.tab-add:hover {
  border-color: var(--ls-primary);
  color: var(--ls-primary);
}

.tiny-button {
  height: 28px;
  padding: 0 10px;
}

.primary-button:disabled,
.ghost-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.modal-mask {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: grid;
  place-items: center;
  background: rgba(15, 23, 42, 0.42);
  backdrop-filter: blur(5px);
}

.connection-manager-dialog {
  display: grid;
  width: min(940px, calc(100vw - 80px));
  height: min(600px, calc(100vh - 80px));
  grid-template-rows: auto auto minmax(0, 1fr);
  overflow: hidden;
  border-radius: 12px;
  box-shadow: var(--ls-shadow-md);
}

.dialog-head {
  min-height: 58px;
  border-bottom: 1px solid var(--ls-border);
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-strong));
  padding: 12px 14px;
}

.manager-toolbar--top {
  display: grid;
  align-items: center;
  grid-template-columns: auto auto minmax(180px, 1fr) 140px;
  gap: 8px;
  border-bottom: 1px solid var(--ls-border);
  background: var(--ls-panel-soft);
  padding: 10px 12px;
}

.manager-toolbar--top .search-control {
  margin: 0;
}

.dialog-body {
  display: grid;
  min-height: 0;
  grid-template-columns: minmax(0, 1fr) 340px;
  gap: 12px;
  padding: 12px;
}

.manager-list {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 10px;
}

.manager-section {
  display: grid;
  gap: 8px;
}

.manager-section--tree {
  min-height: 0;
  overflow: auto;
}

.manager-section h4 {
  color: var(--ls-text);
  font-size: 13px;
}

.manager-section__head span {
  color: var(--ls-text-muted);
  font-size: 12px;
}

.manager-host,
.manager-host--recent {
  grid-template-columns: minmax(0, 1fr) 140px 72px;
}

.connection-tree {
  display: grid;
  gap: 10px;
}

.tree-group {
  display: grid;
  gap: 6px;
}

.tree-group__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--ls-text);
  font-size: 12px;
  font-weight: 700;
}

.tree-group__head em {
  display: inline-grid;
  min-width: 22px;
  height: 20px;
  place-items: center;
  border: 1px solid var(--ls-border);
  border-radius: 999px;
  background: var(--ls-panel-soft);
  color: var(--ls-text-muted);
  font-style: normal;
  font-weight: 600;
}

.tree-host {
  margin-left: 14px;
}

.manager-editor {
  overflow: auto;
}

@media (max-width: 1100px) {
  .quick-connect-page {
    grid-template-columns: minmax(0, 1fr);
    overflow: auto;
  }

  .dialog-body,
  .manager-toolbar--top {
    grid-template-columns: minmax(0, 1fr);
  }

  .connection-manager-dialog {
    height: min(720px, calc(100vh - 48px));
  }
}
</style>
