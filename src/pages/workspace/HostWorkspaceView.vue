<template>
  <main class="workspace-shell">
    <header class="titlebar">
      <div class="brand-block">
        <div class="brand-mark">L</div>
        <div>
          <h1>LiteShell</h1>
          <p>轻量 SSH / SFTP 客户端</p>
        </div>
      </div>

      <div class="host-tabs">
        <button class="host-tab" :class="{ 'host-tab--active': workspaceStore.hasActiveHost }" type="button">
          <span class="online-dot" :class="{ 'online-dot--muted': !workspaceStore.hasActiveHost }"></span>
          <span>{{ activeHostLabel }}</span>
          <span class="auth-state">{{ credentialLabel }}</span>
          <span class="tab-close">×</span>
        </button>
        <button class="tab-add" type="button">+</button>
      </div>

      <div class="window-actions" aria-label="window actions">
        <button type="button">☰</button>
        <button type="button">—</button>
        <button type="button">□</button>
        <button type="button">×</button>
      </div>
    </header>

    <section class="workspace-body">
      <nav class="icon-rail" aria-label="workspace navigation">
        <button class="rail-item rail-item--active" title="终端" type="button">⌁</button>
        <button class="rail-item" title="文件" type="button">□</button>
        <button class="rail-item" title="工具箱" type="button">▣</button>
        <button class="rail-item" title="设置" type="button">⚙</button>
        <span class="rail-spacer"></span>
        <button class="rail-item" title="信息" type="button">i</button>
        <button class="rail-item" title="主题" type="button">◐</button>
      </nav>

      <aside class="monitor-panel">
        <MonitorPanel />
      </aside>

      <section class="workspace-main">
        <section class="panel terminal-panel">
          <header class="panel-head">
            <div><span class="online-dot" :class="{ 'online-dot--muted': !workspaceStore.hasActiveHost }"></span><strong>终端</strong></div>
            <div class="panel-actions"><span class="status-chip">{{ workspaceStore.hasActiveHost ? '已连接' : '未连接' }}</span><span class="status-chip">{{ credentialLabel }}</span><span class="status-chip">SSH</span><span>{{ activeUserLabel }}</span><button type="button">⚡</button><button type="button">⧉</button><button type="button">⋯</button></div>
          </header>
          <div class="workspace-terminal"><TerminalView /></div>
          <div class="terminal-hints">命令提示：Ctrl + Shift + V 粘贴剪贴板　|　Alt + ↑/↓ 历史命令　|　Ctrl + L 清屏</div>
        </section>

        <div class="splitter"><span>···</span></div>

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
import { computed } from 'vue';

import SftpView from '@/pages/sftp/SftpView.vue';
import TerminalView from '@/pages/terminal/TerminalView.vue';
import MonitorPanel from '@/pages/workspace/components/MonitorPanel.vue';
import { useWorkspaceStore } from '@/stores/workspace';

const workspaceStore = useWorkspaceStore();

const activeHostLabel = computed(() => workspaceStore.activeHostLabel);
const credentialLabel = computed(() =>
  workspaceStore.activeHostHasCredential ? '认证：内存' : '认证：未缓存',
);
const activeUserLabel = computed(() => {
  const host = workspaceStore.activeHost;
  if (!host) return '未连接';
  return `${host.username}@${host.host}`;
});
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
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 8px;
  padding: 8px;
}

.workspace-terminal :deep(.connect-card),
.workspace-terminal :deep(.host-list-card),
.workspace-terminal :deep(.quick-card) {
  border-color: rgba(148, 163, 184, 0.12);
  border-radius: 10px;
  background: rgba(2, 6, 23, 0.54);
  padding: 10px;
}

.workspace-terminal :deep(.workspace-card),
.workspace-sftp :deep(.browser-card) {
  border-color: rgba(148, 163, 184, 0.12);
  border-radius: 10px;
  background: rgba(2, 6, 23, 0.68);
}

.workspace-terminal :deep(.terminal-card) {
  padding: 8px;
}

.workspace-terminal :deep(.tabs-bar) {
  min-height: 36px;
  padding: 5px 8px;
}

.workspace-sftp :deep(.content-grid) {
  height: 100%;
  grid-template-columns: minmax(0, 1fr);
  padding: 8px;
}

.workspace-sftp :deep(.path-bar) {
  padding: 8px;
}

@media (max-width: 1260px) {
  .workspace-terminal :deep(.content-grid) {
    grid-template-columns: 220px minmax(0, 1fr);
  }
}
</style>
