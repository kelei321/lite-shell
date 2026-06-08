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
        <button class="host-tab host-tab--active" type="button">
          <span class="online-dot"></span>
          <span>{{ activeHostLabel }}</span>
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
        <section class="system-card system-summary">
          <h2>系统信息</h2>
          <dl>
            <div><dt>主机名</dt><dd>localhost</dd></div>
            <div><dt>IP 地址</dt><dd>{{ activeHostLabel }}</dd></div>
            <div><dt>运行时间</dt><dd>187 天 02:40</dd></div>
            <div><dt>连接用户</dt><dd>root</dd></div>
            <div><dt>系统</dt><dd>CentOS Linux 7.9</dd></div>
            <div><dt>内核</dt><dd>3.10.0-1160.el7.x86_64</dd></div>
            <div><dt>架构</dt><dd>x86_64</dd></div>
          </dl>
        </section>

        <section class="metric-card">
          <div class="metric-head"><span>CPU</span><strong>16 核</strong></div>
          <div class="metric-body">
            <div class="ring ring--blue">14%</div>
            <div class="metric-values">
              <span>用户 11.2%</span>
              <span>系统 2.8%</span>
              <span>空闲 86.0%</span>
            </div>
          </div>
          <div class="sparkline sparkline--blue"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>
        </section>

        <section class="metric-card">
          <div class="metric-head"><span>内存</span><strong>31.1 GiB</strong></div>
          <div class="metric-body">
            <div class="ring ring--cyan">66%</div>
            <div class="metric-values">
              <span>已用 20.6 GiB</span>
              <span>可用 10.5 GiB</span>
              <span>总计 31.1 GiB</span>
            </div>
          </div>
          <div class="progress"><span style="width: 66%"></span></div>
        </section>

        <section class="metric-card">
          <div class="metric-head"><span>交换</span><strong>4.0 GiB</strong></div>
          <div class="metric-body">
            <div class="ring ring--green">25%</div>
            <div class="metric-values">
              <span>已用 1.0 GiB</span>
              <span>可用 3.0 GiB</span>
              <span>总计 4.0 GiB</span>
            </div>
          </div>
        </section>

        <section class="metric-card">
          <div class="metric-head"><span>网络</span><strong>ens33</strong></div>
          <div class="network-row">
            <span class="down">↓ 1.7 KB/s</span>
            <span class="up">↑ 1.2 KB/s</span>
          </div>
          <div class="sparkline sparkline--green"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>
        </section>

        <section class="disk-card">
          <div class="metric-head"><span>磁盘</span><strong>使用率</strong></div>
          <div v-for="disk in disks" :key="disk.path" class="disk-row">
            <span>{{ disk.path }}</span>
            <span>{{ disk.used }}</span>
            <em :style="{ width: disk.percent }"></em>
          </div>
        </section>
      </aside>

      <section class="workspace-main">
        <section class="panel terminal-panel">
          <header class="panel-head">
            <div>
              <span class="online-dot"></span>
              <strong>终端</strong>
            </div>
            <div class="panel-actions">
              <span class="status-chip">已连接</span>
              <span class="status-chip">SSH</span>
              <span>root@{{ activeHostLabel }}</span>
              <button type="button">⚡</button>
              <button type="button">⧉</button>
              <button type="button">⋯</button>
            </div>
          </header>
          <div class="workspace-terminal">
            <TerminalView />
          </div>
          <div class="terminal-hints">命令提示：Ctrl + Shift + V 粘贴剪贴板　|　Alt + ↑/↓ 历史命令　|　Ctrl + L 清屏</div>
        </section>

        <div class="splitter"><span>···</span></div>

        <section class="panel sftp-panel">
          <header class="panel-head">
            <div>
              <span class="online-dot"></span>
              <strong>SFTP 文件管理器</strong>
            </div>
            <div class="panel-actions file-actions">
              <button type="button">←</button>
              <button type="button">↑</button>
              <button type="button">↻</button>
              <button type="button">⇧</button>
              <button type="button">新建</button>
              <button type="button">删除</button>
              <input placeholder="搜索文件" />
            </div>
          </header>
          <div class="workspace-sftp">
            <SftpView />
          </div>
        </section>
      </section>
    </section>

    <footer class="statusbar">
      <span>LiteShell 1.0.0</span>
      <span class="pro-badge">专业版</span>
      <span>连接：1</span>
      <span>传输：↑ 1.2 KB/s ↓ 1.7 KB/s</span>
      <span>SSH 加密：AES-256-CTR 🔒</span>
      <span>会话保活：● 60s</span>
      <span class="statusbar-spacer"></span>
      <span>快捷命令</span>
      <span>工具箱</span>
      <span>设置</span>
    </footer>
  </main>
</template>

<script setup lang="ts">
import SftpView from '@/pages/sftp/SftpView.vue';
import TerminalView from '@/pages/terminal/TerminalView.vue';

const activeHostLabel = '192.168.3.244';

const disks = [
  { path: '/', used: '15.6G / 19.6G', percent: '79%' },
  { path: '/dev', used: '0 / 15.6G', percent: '0%' },
  { path: '/dev/shm', used: '768K / 15.6G', percent: '1%' },
  { path: '/run', used: '1.1G / 15.6G', percent: '7%' },
  { path: '/boot', used: '825M / 1014M', percent: '81%' },
];
</script>

<style scoped>
.workspace-shell {
  display: grid;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  grid-template-rows: 56px minmax(0, 1fr) 44px;
  background:
    radial-gradient(circle at 65% 10%, rgba(37, 99, 235, 0.13), transparent 34%),
    linear-gradient(135deg, #07111d 0%, #020617 55%, #050a13 100%);
  color: #e5e7eb;
}

.titlebar {
  display: grid;
  align-items: center;
  grid-template-columns: 280px minmax(0, 1fr) 190px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.16);
  background: rgba(7, 15, 27, 0.86);
  padding: 0 16px;
  backdrop-filter: blur(12px);
}

.brand-block,
.host-tabs,
.window-actions,
.panel-head,
.panel-actions,
.metric-head,
.metric-body,
.network-row,
.statusbar {
  display: flex;
  align-items: center;
}

.brand-block {
  gap: 10px;
}

.brand-mark {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 10px;
  background: linear-gradient(135deg, #2563eb, #38bdf8);
  box-shadow: 0 8px 18px rgba(37, 99, 235, 0.32);
  color: #fff;
  font-weight: 800;
}

.brand-block h1 {
  margin: 0;
  font-size: 18px;
}

.brand-block p {
  margin: 2px 0 0;
  color: #64748b;
  font-size: 11px;
}

.host-tabs {
  gap: 8px;
  min-width: 0;
}

.host-tab,
.tab-add,
.window-actions button,
.panel-actions button,
.rail-item {
  border: 1px solid rgba(148, 163, 184, 0.18);
  background: rgba(15, 23, 42, 0.86);
  color: #cbd5e1;
  cursor: pointer;
}

.host-tab {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  border-radius: 8px;
  padding: 0 12px;
}

.host-tab--active {
  background: rgba(30, 41, 59, 0.9);
  color: #fff;
}

.online-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: #22c55e;
  box-shadow: 0 0 10px rgba(34, 197, 94, 0.72);
}

.tab-close {
  color: #94a3b8;
  font-size: 16px;
}

.tab-add {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  font-size: 20px;
}

.window-actions {
  justify-content: flex-end;
  gap: 8px;
}

.window-actions button {
  width: 30px;
  height: 28px;
  border-radius: 8px;
}

.workspace-body {
  display: grid;
  min-height: 0;
  grid-template-columns: 52px 292px minmax(0, 1fr);
  gap: 10px;
  padding: 10px;
}

.icon-rail {
  display: flex;
  align-items: center;
  flex-direction: column;
  gap: 12px;
  border: 1px solid rgba(148, 163, 184, 0.13);
  border-radius: 14px;
  background: rgba(15, 23, 42, 0.76);
  padding: 10px 8px;
}

.rail-item {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border-radius: 10px;
  font-size: 16px;
}

.rail-item--active,
.rail-item:hover {
  border-color: rgba(59, 130, 246, 0.64);
  background: rgba(37, 99, 235, 0.22);
  color: #bfdbfe;
}

.rail-spacer {
  flex: 1;
}

.monitor-panel {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 10px;
  overflow: auto;
  border: 1px solid rgba(148, 163, 184, 0.13);
  border-radius: 14px;
  background: rgba(15, 23, 42, 0.7);
  padding: 12px;
}

.system-card,
.metric-card,
.disk-card,
.panel {
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 12px;
  background: linear-gradient(180deg, rgba(15, 23, 42, 0.82), rgba(8, 13, 23, 0.88));
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.2);
}

.system-summary {
  padding: 12px;
}

.system-summary h2 {
  margin: 0 0 12px;
  font-size: 16px;
}

.system-summary dl {
  display: grid;
  gap: 6px;
  margin: 0;
}

.system-summary div {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 6px;
}

.system-summary dt {
  color: #64748b;
  font-size: 12px;
}

.system-summary dd {
  overflow: hidden;
  margin: 0;
  color: #dbeafe;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metric-card,
.disk-card {
  padding: 10px;
}

.metric-head {
  justify-content: space-between;
  color: #cbd5e1;
  font-size: 13px;
}

.metric-head strong {
  color: #e2e8f0;
  font-size: 12px;
  font-weight: 600;
}

.metric-body {
  gap: 12px;
  margin-top: 10px;
}

.ring {
  display: grid;
  width: 58px;
  height: 58px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 999px;
  color: #f8fafc;
  font-size: 13px;
  font-weight: 700;
}

.ring--blue {
  background: conic-gradient(#3b82f6 14%, #1e293b 0);
}

.ring--cyan {
  background: conic-gradient(#38bdf8 66%, #1e293b 0);
}

.ring--green {
  background: conic-gradient(#34d399 25%, #1e293b 0);
}

.metric-values {
  display: grid;
  gap: 4px;
  color: #94a3b8;
  font-size: 12px;
}

.sparkline {
  display: flex;
  align-items: end;
  gap: 4px;
  height: 42px;
  margin-top: 8px;
}

.sparkline i {
  display: block;
  flex: 1;
  border-radius: 999px 999px 0 0;
  background: #2563eb;
  opacity: 0.75;
}

.sparkline i:nth-child(1) { height: 30%; }
.sparkline i:nth-child(2) { height: 48%; }
.sparkline i:nth-child(3) { height: 44%; }
.sparkline i:nth-child(4) { height: 62%; }
.sparkline i:nth-child(5) { height: 36%; }
.sparkline i:nth-child(6) { height: 72%; }
.sparkline i:nth-child(7) { height: 42%; }

.sparkline--green i {
  background: #22c55e;
}

.progress {
  height: 4px;
  overflow: hidden;
  margin-top: 10px;
  border-radius: 999px;
  background: #1e293b;
}

.progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #2563eb, #38bdf8);
}

.network-row {
  justify-content: space-between;
  margin-top: 10px;
  color: #cbd5e1;
  font-size: 12px;
}

.down { color: #22c55e; }
.up { color: #60a5fa; }

.disk-row {
  display: grid;
  align-items: center;
  grid-template-columns: 48px 1fr 48px;
  gap: 8px;
  margin-top: 8px;
  color: #cbd5e1;
  font-size: 11px;
}

.disk-row em {
  display: block;
  height: 4px;
  border-radius: 999px;
  background: linear-gradient(90deg, #22c55e, #f97316);
}

.workspace-main {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: minmax(280px, 1fr) 10px minmax(260px, 0.94fr);
}

.panel {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
}

.panel-head {
  justify-content: space-between;
  min-height: 40px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.14);
  padding: 0 12px;
}

.panel-head > div:first-child {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.panel-actions {
  gap: 8px;
  color: #94a3b8;
  font-size: 12px;
}

.panel-actions button {
  height: 26px;
  border-radius: 8px;
  padding: 0 8px;
}

.status-chip {
  display: inline-flex;
  align-items: center;
  height: 24px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 999px;
  background: rgba(15, 23, 42, 0.78);
  color: #dbeafe;
  padding: 0 9px;
}

.workspace-terminal,
.workspace-sftp {
  min-width: 0;
  min-height: 0;
  flex: 1;
  overflow: hidden;
}

.terminal-hints {
  position: absolute;
  right: 22px;
  bottom: 16px;
  border-radius: 999px;
  background: rgba(2, 6, 23, 0.7);
  color: #94a3b8;
  padding: 6px 12px;
  font-size: 12px;
  pointer-events: none;
}

.terminal-panel {
  position: relative;
}

.splitter {
  display: grid;
  place-items: center;
  color: #94a3b8;
}

.splitter span {
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.28);
  padding: 0 18px;
  line-height: 10px;
}

.file-actions input {
  width: 170px;
  height: 26px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 8px;
  outline: none;
  background: rgba(2, 6, 23, 0.76);
  color: #e5e7eb;
  padding: 0 9px;
}

.statusbar {
  gap: 28px;
  border-top: 1px solid rgba(148, 163, 184, 0.15);
  background: rgba(15, 23, 42, 0.82);
  color: #94a3b8;
  padding: 0 16px;
  font-size: 12px;
}

.pro-badge {
  border-radius: 6px;
  background: #16a34a;
  color: #fff;
  padding: 2px 8px;
}

.statusbar-spacer {
  flex: 1;
}

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

.workspace-terminal :deep(.content-grid),
.workspace-sftp :deep(.content-grid) {
  height: 100%;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 8px;
  padding: 8px;
}

.workspace-terminal :deep(.host-panel),
.workspace-sftp :deep(.host-panel) {
  min-width: 0;
  gap: 8px;
}

.workspace-terminal :deep(.connect-card),
.workspace-terminal :deep(.host-list-card),
.workspace-terminal :deep(.quick-card),
.workspace-sftp :deep(.connect-card),
.workspace-sftp :deep(.host-list-card) {
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

.workspace-sftp :deep(.path-bar) {
  padding: 8px;
}

@media (max-width: 1260px) {
  .workspace-body {
    grid-template-columns: 48px 240px minmax(0, 1fr);
  }

  .workspace-terminal :deep(.content-grid),
  .workspace-sftp :deep(.content-grid) {
    grid-template-columns: 220px minmax(0, 1fr);
  }
}
</style>
