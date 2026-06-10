<template>
  <section class="terminal-layout">
    <header class="toolbar">
      <div>
        <h2>SSH 终端</h2>
        <p>主机信息本地保存，密码不落盘；连接成功后同步到工作台当前主机。</p>
      </div>
      <div class="status" :class="{ 'status--online': activeTab?.sessionId }">
        {{ activeTab ? activeTab.statusText : '无会话' }}
      </div>
    </header>

    <div class="content-grid">
      <aside class="host-panel">
        <form class="connect-card" @submit.prevent="saveAndConnect">
          <div class="panel-title-row">
            <h3>主机配置</h3>
            <button class="tiny-button" type="button" @click="resetForm">新增</button>
          </div>

          <label><span>名称</span><input v-model.trim="form.name" autocomplete="off" placeholder="生产服务器" /></label>
          <label><span>分组</span><input v-model.trim="form.group" autocomplete="off" list="host-group-options" placeholder="默认分组" /></label>
          <datalist id="host-group-options">
            <option v-for="group in hostStore.groups" :key="group" :value="group"></option>
          </datalist>
          <label><span>主机</span><input v-model.trim="form.host" autocomplete="off" placeholder="127.0.0.1" /></label>
          <label><span>端口</span><input v-model.number="form.port" min="1" max="65535" type="number" /></label>
          <label><span>用户名</span><input v-model.trim="form.username" autocomplete="username" placeholder="root" /></label>
          <label><span>密码</span><input v-model="form.password" autocomplete="current-password" type="password" /></label>

          <div class="action-row">
            <button class="primary-button" :disabled="!canConnect || connecting" type="submit">
              {{ connecting ? '连接中...' : '保存并连接' }}
            </button>
            <button class="ghost-button" :disabled="!canSave" type="button" @click="saveHost">保存</button>
          </div>
        </form>

        <section class="host-list-card">
          <div class="panel-title-row">
            <h3>主机列表</h3>
            <span class="host-count">{{ filteredHosts.length }}/{{ hostStore.sortedHosts.length }}</span>
          </div>

          <input v-model.trim="hostSearch" class="host-search" autocomplete="off" placeholder="搜索名称、地址、用户、分组" />

          <div v-if="hostStore.recentHosts.length" class="recent-strip">
            <span>最近</span>
            <button v-for="host in hostStore.recentHosts" :key="host.id" type="button" @click="selectHost(host)">
              {{ host.name }}
            </button>
          </div>

          <div class="group-filter-row">
            <button
              class="group-filter"
              :class="{ 'group-filter--active': selectedGroup === '' }"
              type="button"
              @click="selectedGroup = ''"
            >
              全部
            </button>
            <button
              v-for="group in hostStore.groups"
              :key="group"
              class="group-filter"
              :class="{ 'group-filter--active': selectedGroup === group }"
              type="button"
              @click="selectedGroup = group"
            >
              {{ group }}
            </button>
          </div>

          <p v-if="hostStore.sortedHosts.length === 0" class="empty-tip">暂无主机，填写上方表单后点击保存。</p>
          <p v-else-if="filteredHosts.length === 0" class="empty-tip">没有匹配的主机。</p>

          <div v-else class="grouped-hosts">
            <section v-for="group in groupedFilteredHosts" :key="group.name" class="host-group">
              <div class="host-group__head">
                <span>{{ group.name }}</span>
                <em>{{ group.hosts.length }}</em>
              </div>
              <button
                v-for="host in group.hosts"
                :key="host.id"
                class="host-item"
                type="button"
                @click="selectHost(host)"
              >
                <span class="host-item__name">{{ host.name }}</span>
                <span class="host-item__meta">{{ host.username }}@{{ host.host }}:{{ host.port }}</span>
                <span v-if="host.lastConnectedAt" class="host-item__recent">最近：{{ formatRecentTime(host.lastConnectedAt) }}</span>
              </button>
            </section>
          </div>
        </section>

        <section class="quick-card">
          <h3>快捷命令</h3>
          <button v-for="command in quickCommands" :key="command" class="command-button" :disabled="!activeTab?.sessionId" type="button" @click="sendCommand(command)">
            {{ command }}
          </button>
        </section>
      </aside>

      <section class="workspace-card">
        <div class="tabs-bar">
          <button v-for="tab in tabs" :key="tab.id" class="tab-item" :class="{ 'tab-item--active': tab.id === activeTabId }" type="button" @click="activateTab(tab.id)">
            <span>{{ tab.title }}</span>
            <span class="tab-dot" :class="{ 'tab-dot--online': tab.sessionId }"></span>
            <span class="tab-close" @click.stop="closeTab(tab.id)">×</span>
          </button>
          <span v-if="tabs.length === 0" class="empty-tabs">暂无终端标签页</span>
        </div>

        <div class="terminal-card">
          <div v-for="tab in tabs" :key="tab.id" :ref="(el) => setTerminalHost(tab.id, el)" class="terminal-host" v-show="tab.id === activeTabId"></div>
          <div v-if="tabs.length === 0" class="welcome-card">
            <h3>LiteShell ready</h3>
            <p>选择或新增一个主机，输入密码后点击“保存并连接”。</p>
            <p>密码只用于本次连接，不会写入 localStorage。</p>
          </div>
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

const hostStore = useHostStore();
const workspaceStore = useWorkspaceStore();
const quickCommands = ['pwd', 'ls -la', 'df -h', 'free -m', 'top'];

const form = reactive({ id: '', name: '', group: '默认分组', host: '127.0.0.1', port: 22, username: 'root', password: '' });
const tabs = ref<TerminalTab[]>([]);
const activeTabId = ref('');
const connecting = ref(false);
const hostSearch = ref('');
const selectedGroup = ref('');
const terminalHosts = new Map<string, HTMLDivElement>();

const activeTab = computed(() => tabs.value.find((tab) => tab.id === activeTabId.value));
const canSave = computed(() => Boolean(form.host.trim() && form.username.trim()));
const canConnect = computed(() => Boolean(canSave.value && form.password));
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
    workspaceStore.clearCredential(tab.hostId);
    tab.terminal?.dispose();
  }
  workspaceStore.clearAllCredentials();
});

function resetForm() {
  form.id = '';
  form.name = '';
  form.group = selectedGroup.value || '默认分组';
  form.host = '127.0.0.1';
  form.port = 22;
  form.username = 'root';
  form.password = '';
}

function selectHost(host: HostProfile) {
  form.id = host.id;
  form.name = host.name;
  form.group = host.group || '默认分组';
  form.host = host.host;
  form.port = host.port;
  form.username = host.username;
  form.password = '';
}

function saveHost() {
  if (!canSave.value) return undefined;
  const host = hostStore.upsertHost({ id: form.id || undefined, name: form.name, group: form.group, host: form.host, port: form.port, username: form.username });
  if (host) selectHost(host);
  return host;
}

async function saveAndConnect() {
  if (!canConnect.value || connecting.value) return;
  const password = form.password;
  const host = saveHost();
  if (!host) return;
  hostStore.touchHost(host.id);
  form.password = '';
  await openTerminalTab(host, password);
}

async function openTerminalTab(host: HostProfile, password: string) {
  const tab: TerminalTab = {
    id: createId(),
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
  if (tab.terminal) return;
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
  if (tab) workspaceStore.setActiveHost({ id: tab.hostId, name: tab.title, host: tab.host, port: tab.port, username: tab.username });
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

  const stillHasSameHostTab = tabs.value.some((item) => item.hostId === tab.hostId);
  if (!stillHasSameHostTab) {
    workspaceStore.clearCredential(tab.hostId);
  }

  if (activeTabId.value === tabId) {
    const next = tabs.value[Math.max(0, index - 1)] || tabs.value[0];
    activeTabId.value = next?.id || '';
    if (next) workspaceStore.setActiveHost({ id: next.hostId, name: next.title, host: next.host, port: next.port, username: next.username });
    else workspaceStore.clearActiveHost(tab.hostId);
    scheduleActiveTabRefresh();
  }
}

function sendCommand(command: string) {
  const tab = activeTab.value;
  if (!tab?.sessionId) return;
  void invoke('ssh_write', { id: tab.sessionId, data: `${command}\n` });
  tab.terminal?.focus();
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
  if (!tab.terminal || !tab.fitAddon) return;
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

function formatRecentTime(timestamp: number) {
  return new Date(timestamp).toLocaleString();
}

function createId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
</script>

<style scoped>
.terminal-layout { display: flex; flex: 1; min-width: 0; flex-direction: column; padding: 18px; gap: 16px; }
.toolbar { display: flex; align-items: center; justify-content: space-between; min-height: 64px; border: 1px solid #1e293b; border-radius: 16px; background: #0f172a; padding: 14px 18px; }
.toolbar h2, .host-list-card h3, .quick-card h3, .panel-title-row h3 { margin: 0; }
.toolbar h2 { font-size: 20px; }
.toolbar p { margin: 6px 0 0; color: #94a3b8; font-size: 13px; }
.status { border-radius: 999px; background: #334155; color: #cbd5e1; padding: 6px 12px; font-size: 13px; }
.status--online { background: rgba(34, 197, 94, 0.14); color: #86efac; }
.content-grid { display: grid; grid-template-columns: 320px minmax(0, 1fr); min-height: 0; flex: 1; gap: 16px; }
.host-panel { display: flex; min-height: 0; flex-direction: column; gap: 12px; }
.connect-card, .host-list-card, .quick-card, .workspace-card { border: 1px solid #1e293b; border-radius: 16px; background: #0f172a; }
.connect-card, .host-list-card, .quick-card { display: flex; flex-direction: column; gap: 12px; padding: 16px; }
.host-list-card { min-height: 0; overflow: auto; }
.panel-title-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.connect-card label { display: grid; gap: 6px; }
.connect-card span { color: #94a3b8; font-size: 12px; }
.connect-card input, .host-search { width: 100%; height: 36px; border: 1px solid #334155; border-radius: 10px; outline: none; background: #020617; color: #e5e7eb; padding: 0 10px; }
.connect-card input:focus, .host-search:focus { border-color: #2563eb; }
.action-row { display: grid; grid-template-columns: 1fr 86px; gap: 10px; margin-top: 4px; }
.primary-button, .ghost-button, .command-button, .tiny-button, .host-item, .tab-item, .group-filter, .recent-strip button { border-radius: 10px; color: #fff; cursor: pointer; }
.primary-button, .ghost-button, .command-button { height: 36px; }
.primary-button { background: #2563eb; }
.ghost-button, .command-button, .tiny-button, .group-filter, .recent-strip button { border: 1px solid #334155; background: #1e293b; }
.tiny-button { height: 28px; padding: 0 10px; color: #cbd5e1; }
.primary-button:disabled, .ghost-button:disabled, .command-button:disabled { opacity: 0.45; cursor: not-allowed; }
.empty-tip, .empty-tabs, .welcome-card p, .host-count { color: #94a3b8; font-size: 13px; }
.recent-strip { display: flex; align-items: center; gap: 6px; overflow-x: auto; }
.recent-strip span { flex: 0 0 auto; color: #94a3b8; font-size: 12px; }
.recent-strip button { height: 26px; flex: 0 0 auto; max-width: 118px; overflow: hidden; color: #cbd5e1; padding: 0 8px; text-overflow: ellipsis; white-space: nowrap; }
.recent-strip button:hover, .group-filter:hover { border-color: #2563eb; }
.group-filter-row { display: flex; flex-wrap: wrap; gap: 6px; }
.group-filter { min-height: 26px; color: #cbd5e1; padding: 0 8px; font-size: 12px; }
.group-filter--active { border-color: #2563eb; background: rgba(37, 99, 235, 0.24); color: #bfdbfe; }
.grouped-hosts { display: grid; gap: 10px; }
.host-group { display: grid; gap: 6px; }
.host-group__head { display: flex; align-items: center; justify-content: space-between; color: #cbd5e1; font-size: 12px; }
.host-group__head em { color: #64748b; font-style: normal; }
.host-item { display: grid; gap: 4px; border: 1px solid #1e293b; background: #111827; padding: 10px; text-align: left; }
.host-item:hover { border-color: #2563eb; }
.host-item__name { color: #f8fafc; font-size: 14px; }
.host-item__meta, .host-item__recent { color: #94a3b8; font-size: 12px; }
.quick-card { flex: 0 0 auto; }
.command-button { text-align: left; padding: 0 10px; }
.workspace-card { display: flex; min-width: 0; min-height: 0; flex-direction: column; overflow: hidden; }
.tabs-bar { display: flex; align-items: center; gap: 8px; min-height: 46px; border-bottom: 1px solid #1e293b; padding: 8px; overflow-x: auto; }
.tab-item { display: inline-flex; align-items: center; gap: 8px; height: 30px; border: 1px solid #334155; background: #111827; padding: 0 8px 0 10px; white-space: nowrap; }
.tab-item--active { border-color: #2563eb; background: #1e293b; }
.tab-dot { width: 7px; height: 7px; border-radius: 999px; background: #64748b; }
.tab-dot--online { background: #22c55e; }
.tab-close { color: #94a3b8; font-size: 16px; line-height: 1; }
.tab-close:hover { color: #fff; }
.terminal-card { position: relative; min-width: 0; min-height: 0; flex: 1; padding: 10px; }
.terminal-host { width: 100%; height: 100%; overflow: hidden; border-radius: 12px; background: #020617; }
.welcome-card { display: grid; height: 100%; place-content: center; text-align: center; }
.welcome-card h3 { margin: 0 0 8px; color: #f8fafc; }
.welcome-card p { margin: 4px 0; }
</style>
