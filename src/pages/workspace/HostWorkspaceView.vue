<template>
  <main class="workspace-shell">
    <section
      class="workspace-body"
      :class="{ 'workspace-body--monitor-collapsed': isMonitorCollapsed }"
      :style="{ gridTemplateColumns: workspaceBodyColumns }"
    >
      <nav class="icon-rail" aria-label="workspace navigation">
        <div class="rail-brand">L</div>
        <button class="rail-item rail-item--active" title="终端" type="button">⌁</button>
        <button class="rail-item" title="文件" type="button">□</button>
        <button class="rail-item" title="工具箱" type="button">▣</button>
        <button class="rail-item" title="设置" type="button">⚙</button>
        <span class="rail-spacer"></span>
        <button class="rail-item" title="信息" type="button">i</button>
        <button class="rail-item" :title="isDarkTheme ? '切换浅色' : '切换暗色'" type="button" @click="toggleTheme">
          {{ isDarkTheme ? '☀' : '◐' }}
        </button>
      </nav>

      <aside v-if="!isMonitorCollapsed" ref="monitorPanelRef" class="monitor-panel">
        <button class="monitor-toggle" title="折叠监控面板" type="button" @click="toggleMonitorPanel">
          ‹
        </button>
        <MonitorPanel />
      </aside>

      <button v-else class="monitor-expand-button" title="展开监控面板" type="button" @click="toggleMonitorPanel">
        ›
      </button>

      <div
        v-if="!isMonitorCollapsed"
        class="workspace-vertical-splitter"
        role="separator"
        aria-label="调整监控面板宽度"
        aria-orientation="vertical"
        @pointerdown="startMonitorResize"
      ></div>

      <section ref="workspaceMainRef" class="workspace-main" :style="{ gridTemplateRows: workspaceMainRows }">
        <section class="panel terminal-panel">
          <header class="panel-head">
            <div><span class="online-dot" :class="{ 'online-dot--muted': !workspaceStore.hasActiveHost }"></span><strong>终端</strong></div>
            <div class="panel-actions"><span class="status-chip">{{ workspaceStore.hasActiveHost ? '已连接' : '未连接' }}</span><span class="status-chip">{{ credentialLabel }}</span><span class="status-chip">SSH</span><span>{{ activeUserLabel }}</span><button type="button">⚡</button><button type="button">⧉</button><button type="button">⋯</button></div>
          </header>
          <div class="workspace-terminal"><TerminalView /></div>
          <div class="terminal-hints">命令提示：Ctrl + Shift + V 粘贴剪贴板　|　Alt + ↑/↓ 历史命令　|　Ctrl + L 清屏</div>
        </section>

        <div
          class="splitter"
          role="separator"
          aria-label="调整终端和 SFTP 高度"
          aria-orientation="horizontal"
          @pointerdown="startPanelResize"
        >
          <span>···</span>
        </div>

        <section class="panel sftp-panel">
          <header class="panel-head">
            <div><span class="online-dot" :class="{ 'online-dot--muted': !workspaceStore.hasActiveHost }"></span><strong>SFTP 文件管理器</strong></div>
            <div class="panel-actions"><span class="status-chip">{{ credentialLabel }}</span><span>{{ activeUserLabel }}</span></div>
          </header>
          <div class="workspace-sftp"><SftpView /></div>
        </section>
      </section>
    </section>

    <footer class="statusbar"><span>LiteShell 1.0.0</span><span class="pro-badge">专业版</span><span>连接：{{ workspaceStore.hasActiveHost ? 1 : 0 }}</span><span>{{ credentialLabel }}</span><span>传输：↑ 1.2 KB/s ↓ 1.7 KB/s</span><span>SSH 加密：AES-256-CTR 🔒</span><span>会话保活：● 60s</span><span class="statusbar-spacer"></span><span>快捷命令</span><span>工具箱</span><span>设置</span></footer>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';

import SftpView from '@/pages/sftp/SftpView.vue';
import TerminalView from '@/pages/terminal/TerminalView.vue';
import MonitorPanel from '@/pages/workspace/components/MonitorPanel.vue';
import { useWorkspaceStore } from '@/stores/workspace';

type WorkspaceLayoutPreference = {
  monitorCollapsed?: boolean;
  monitorWidth?: number;
  terminalRatio?: number;
};

type AppTheme = 'light' | 'dark';

const LAYOUT_STORAGE_KEY = 'lite-shell.workspace.layout';
const THEME_STORAGE_KEY = 'lite-shell.theme';
const DEFAULT_MONITOR_WIDTH = 260;
const COLLAPSED_EXPAND_WIDTH = 28;
const MIN_MONITOR_WIDTH = 238;
const MAX_MONITOR_WIDTH = 360;
const DEFAULT_TERMINAL_RATIO = 0.52;
const MIN_PANEL_RATIO = 0.28;
const MAX_PANEL_RATIO = 0.72;

const workspaceStore = useWorkspaceStore();
const workspaceMainRef = ref<HTMLElement | null>(null);
const monitorPanelRef = ref<HTMLElement | null>(null);
const terminalRatio = ref(DEFAULT_TERMINAL_RATIO);
const monitorWidth = ref(DEFAULT_MONITOR_WIDTH);
const isMonitorCollapsed = ref(false);
const resizingPanel = ref(false);
const resizingMonitor = ref(false);
const theme = ref<AppTheme>('light');

const credentialLabel = computed(() =>
  workspaceStore.activeHostHasCredential ? '认证：内存' : '认证：未缓存',
);
const activeUserLabel = computed(() => {
  const host = workspaceStore.activeHost;
  if (!host) return '未连接';
  return `${host.username}@${host.host}`;
});
const isDarkTheme = computed(() => theme.value === 'dark');
const workspaceMainRows = computed(() => {
  const topRatio = terminalRatio.value.toFixed(3);
  const bottomRatio = (1 - terminalRatio.value).toFixed(3);
  return `minmax(180px, ${topRatio}fr) 10px minmax(160px, ${bottomRatio}fr)`;
});
const workspaceBodyColumns = computed(() => {
  if (isMonitorCollapsed.value) {
    return `52px ${COLLAPSED_EXPAND_WIDTH}px minmax(0, 1fr)`;
  }

  return `52px ${monitorWidth.value}px 8px minmax(0, 1fr)`;
});

onMounted(() => {
  restoreThemePreference();
  restoreLayoutPreference();
});

onBeforeUnmount(() => {
  stopPanelResize();
  stopMonitorResize();
});

function toggleTheme() {
  applyTheme(isDarkTheme.value ? 'light' : 'dark');
}

function applyTheme(nextTheme: AppTheme) {
  theme.value = nextTheme;
  document.documentElement.dataset.theme = nextTheme;

  try {
    localStorage.setItem(THEME_STORAGE_KEY, nextTheme);
  } catch {
    // Ignore localStorage quota or privacy-mode failures.
  }
}

function restoreThemePreference() {
  try {
    const value = localStorage.getItem(THEME_STORAGE_KEY);
    applyTheme(value === 'dark' ? 'dark' : 'light');
  } catch {
    applyTheme('light');
  }
}

function toggleMonitorPanel() {
  isMonitorCollapsed.value = !isMonitorCollapsed.value;
  saveLayoutPreference();
}

function startPanelResize(event: PointerEvent) {
  if (resizingPanel.value) return;

  event.preventDefault();
  resizingPanel.value = true;
  document.body.classList.add('is-resizing-row');
  window.addEventListener('pointermove', handlePanelResize);
  window.addEventListener('pointerup', stopPanelResize);
  window.addEventListener('pointercancel', stopPanelResize);
}

function handlePanelResize(event: PointerEvent) {
  if (!resizingPanel.value || !workspaceMainRef.value) return;

  const rect = workspaceMainRef.value.getBoundingClientRect();
  const availableHeight = Math.max(1, rect.height - 10);
  const nextRatio = clamp((event.clientY - rect.top) / availableHeight, MIN_PANEL_RATIO, MAX_PANEL_RATIO);
  terminalRatio.value = Number(nextRatio.toFixed(3));
}

function stopPanelResize() {
  if (!resizingPanel.value) return;

  resizingPanel.value = false;
  document.body.classList.remove('is-resizing-row');
  window.removeEventListener('pointermove', handlePanelResize);
  window.removeEventListener('pointerup', stopPanelResize);
  window.removeEventListener('pointercancel', stopPanelResize);
  saveLayoutPreference();
}

function startMonitorResize(event: PointerEvent) {
  if (isMonitorCollapsed.value || resizingMonitor.value) return;

  event.preventDefault();
  resizingMonitor.value = true;
  document.body.classList.add('is-resizing-column');
  window.addEventListener('pointermove', handleMonitorResize);
  window.addEventListener('pointerup', stopMonitorResize);
  window.addEventListener('pointercancel', stopMonitorResize);
}

function handleMonitorResize(event: PointerEvent) {
  if (!resizingMonitor.value || !monitorPanelRef.value) return;

  const rect = monitorPanelRef.value.getBoundingClientRect();
  monitorWidth.value = Math.round(clamp(event.clientX - rect.left, MIN_MONITOR_WIDTH, MAX_MONITOR_WIDTH));
}

function stopMonitorResize() {
  if (!resizingMonitor.value) return;

  resizingMonitor.value = false;
  document.body.classList.remove('is-resizing-column');
  window.removeEventListener('pointermove', handleMonitorResize);
  window.removeEventListener('pointerup', stopMonitorResize);
  window.removeEventListener('pointercancel', stopMonitorResize);
  saveLayoutPreference();
}

function restoreLayoutPreference() {
  const preference = readLayoutPreference();

  if (!preference) return;

  if (typeof preference.terminalRatio === 'number') {
    terminalRatio.value = clamp(preference.terminalRatio, MIN_PANEL_RATIO, MAX_PANEL_RATIO);
  }

  if (typeof preference.monitorWidth === 'number') {
    monitorWidth.value = Math.round(clamp(preference.monitorWidth, MIN_MONITOR_WIDTH, MAX_MONITOR_WIDTH));
  }

  if (typeof preference.monitorCollapsed === 'boolean') {
    isMonitorCollapsed.value = preference.monitorCollapsed;
  }
}

function readLayoutPreference(): WorkspaceLayoutPreference | null {
  try {
    const rawValue = localStorage.getItem(LAYOUT_STORAGE_KEY);
    if (!rawValue) return null;

    return JSON.parse(rawValue) as WorkspaceLayoutPreference;
  } catch {
    return null;
  }
}

function saveLayoutPreference() {
  const preference: WorkspaceLayoutPreference = {
    monitorCollapsed: isMonitorCollapsed.value,
    monitorWidth: monitorWidth.value,
    terminalRatio: terminalRatio.value,
  };

  try {
    localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(preference));
  } catch {
    // Ignore localStorage quota or privacy-mode failures.
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
</script>

<style scoped>
.workspace-terminal :deep(.terminal-layout),
.workspace-sftp :deep(.sftp-layout) {
  height: 100%;
  padding: 0;
  gap: 0;
}

.workspace-terminal :deep(.toolbar),
.workspace-sftp :deep(.toolbar) {
  display: none;
}

.workspace-terminal :deep(.content-grid) {
  height: 100%;
  grid-template-columns: minmax(0, 1fr);
  padding: 8px;
}

.workspace-terminal :deep(.workspace-card),
.workspace-sftp :deep(.browser-card) {
  border-color: var(--ls-border);
  border-radius: 10px;
}

.workspace-terminal :deep(.terminal-card) {
  padding: 8px;
}

.workspace-terminal :deep(.tabs-bar) {
  min-height: 34px;
  padding: 4px 8px;
}

.workspace-terminal :deep(.quick-connect-page) {
  min-height: 0;
}

.workspace-terminal :deep(.connection-manager-dialog) {
  max-height: calc(100vh - 80px);
}

.workspace-sftp :deep(.content-grid) {
  height: 100%;
  grid-template-columns: minmax(0, 1fr);
  padding: 8px;
}

.workspace-sftp :deep(.path-bar) {
  padding: 8px;
}

.rail-brand {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border-radius: 10px;
  background: linear-gradient(135deg, var(--ls-primary), #38bdf8);
  box-shadow: var(--ls-shadow-sm);
  color: #fff;
  font-weight: 800;
}

.monitor-panel {
  min-width: 0;
}

.monitor-toggle,
.monitor-expand-button {
  display: grid;
  width: 24px;
  height: 24px;
  place-items: center;
  border: 1px solid var(--ls-border-strong);
  border-radius: 7px;
  background: linear-gradient(180deg, var(--ls-panel), var(--ls-panel-strong));
  color: var(--ls-text-muted);
  cursor: pointer;
}

.monitor-toggle {
  flex: 0 0 auto;
  align-self: flex-end;
  margin-bottom: 4px;
}

.monitor-expand-button {
  align-self: start;
  justify-self: center;
  margin-top: 4px;
}

.monitor-toggle:hover,
.monitor-expand-button:hover {
  border-color: var(--ls-primary);
  color: var(--ls-primary);
}

.workspace-vertical-splitter {
  display: grid;
  min-width: 0;
  place-items: center;
  border-radius: 999px;
  cursor: col-resize;
}

.workspace-vertical-splitter::before {
  display: block;
  width: 4px;
  height: 54px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--ls-border-strong) 45%, transparent);
  content: '';
}

.workspace-vertical-splitter:hover::before {
  background: color-mix(in srgb, var(--ls-primary) 64%, transparent);
}

.splitter {
  cursor: row-resize;
}

.splitter:hover span {
  background: color-mix(in srgb, var(--ls-primary) 52%, transparent);
  color: var(--ls-primary);
}

:global(body.is-resizing-row),
:global(body.is-resizing-row *) {
  cursor: row-resize !important;
  user-select: none;
}

:global(body.is-resizing-column),
:global(body.is-resizing-column *) {
  cursor: col-resize !important;
  user-select: none;
}
</style>
