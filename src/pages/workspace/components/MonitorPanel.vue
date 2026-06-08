<template>
  <div class="monitor-stack">
    <div v-if="statusMessage" class="monitor-empty">{{ statusMessage }}</div>

    <template v-else>
      <section class="monitor-card">
        <div class="monitor-card__head">
          <h2>系统信息</h2>
          <span>{{ refreshLabel }}</span>
        </div>
        <dl class="monitor-grid">
          <div class="monitor-row">
            <dt>主机名</dt>
            <dd>{{ valueOrDash(snapshot?.hostname) }}</dd>
          </div>
          <div class="monitor-row">
            <dt>IP 地址</dt>
            <dd>{{ activeHost?.host || '-' }}</dd>
          </div>
          <div class="monitor-row">
            <dt>连接用户</dt>
            <dd>{{ activeHost?.username || '-' }}</dd>
          </div>
          <div class="monitor-row">
            <dt>系统</dt>
            <dd>{{ valueOrDash(snapshot?.os) }}</dd>
          </div>
          <div class="monitor-row">
            <dt>内核</dt>
            <dd>{{ valueOrDash(snapshot?.kernel) }}</dd>
          </div>
          <div class="monitor-row">
            <dt>运行时间</dt>
            <dd>{{ valueOrDash(snapshot?.uptime) }}</dd>
          </div>
        </dl>
      </section>

      <section class="monitor-card">
        <div class="monitor-card__head">
          <h3>CPU</h3>
          <strong>{{ formatPercent(snapshot?.cpuUsage) }}</strong>
        </div>
        <div class="monitor-progress">
          <span class="monitor-progress__bar monitor-progress__bar--cpu" :style="{ width: progressWidth(snapshot?.cpuUsage) }"></span>
        </div>
      </section>

      <section class="monitor-card">
        <div class="monitor-card__head">
          <h3>内存</h3>
          <strong>{{ formatPercent(snapshot?.memory.usagePercent) }}</strong>
        </div>
        <p class="monitor-muted">{{ formatMemory(snapshot?.memory.usedMb) }} / {{ formatMemory(snapshot?.memory.totalMb) }}</p>
        <div class="monitor-progress">
          <span class="monitor-progress__bar monitor-progress__bar--memory" :style="{ width: progressWidth(snapshot?.memory.usagePercent) }"></span>
        </div>
      </section>

      <section class="monitor-card">
        <div class="monitor-card__head">
          <h3>Swap</h3>
          <strong>{{ formatPercent(snapshot?.swap.usagePercent) }}</strong>
        </div>
        <p class="monitor-muted">{{ formatMemory(snapshot?.swap.usedMb) }} / {{ formatMemory(snapshot?.swap.totalMb) }}</p>
        <div class="monitor-progress">
          <span class="monitor-progress__bar monitor-progress__bar--swap" :style="{ width: progressWidth(snapshot?.swap.usagePercent) }"></span>
        </div>
      </section>

      <section class="monitor-card">
        <div class="monitor-card__head">
          <h3>网络</h3>
          <span>{{ snapshot?.networks.length || 0 }} 个网卡</span>
        </div>
        <div v-if="snapshot?.networks.length" class="monitor-list">
          <div v-for="network in snapshot.networks" :key="network.name" class="network-item">
            <span class="network-name">{{ network.name }}</span>
            <span class="network-rate network-rate--down">↓ {{ getNetworkRate(network.name, 'rx') }}</span>
            <span class="network-rate network-rate--up">↑ {{ getNetworkRate(network.name, 'tx') }}</span>
          </div>
        </div>
        <p v-else class="monitor-muted">-</p>
      </section>

      <section class="monitor-card">
        <div class="monitor-card__head">
          <h3>磁盘</h3>
          <span>使用率</span>
        </div>
        <div v-if="snapshot?.disks.length" class="monitor-list">
          <div v-for="disk in snapshot.disks" :key="`${disk.filesystem}-${disk.mount}`" class="disk-item">
            <div class="disk-item__head">
              <span>{{ disk.mount || '-' }}</span>
              <strong>{{ formatPercent(disk.usagePercent) }}</strong>
            </div>
            <p class="monitor-muted">{{ disk.used || '-' }} / {{ disk.total || '-' }} · 可用 {{ disk.available || '-' }}</p>
            <div class="monitor-progress">
              <span class="monitor-progress__bar monitor-progress__bar--disk" :style="{ width: progressWidth(disk.usagePercent) }"></span>
            </div>
          </div>
        </div>
        <p v-else class="monitor-muted">-</p>
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

import { useWorkspaceStore } from '@/stores/workspace';
import type { MonitorSnapshot } from '@/types/monitor';

const workspaceStore = useWorkspaceStore();

const snapshot = ref<MonitorSnapshot | null>(null);
const previousSnapshot = ref<MonitorSnapshot | null>(null);
const loading = ref(false);
const errorMessage = ref('');

let timer: number | undefined;
let requestVersion = 0;

const activeHost = computed(() => workspaceStore.activeHost);
const statusMessage = computed(() => {
  if (!activeHost.value) return '请先连接 SSH 主机';
  if (!workspaceStore.hasCredential(activeHost.value.id)) return '当前主机未缓存认证';
  if (loading.value && !snapshot.value) return '监控加载中...';
  if (errorMessage.value) return errorMessage.value;
  return '';
});
const refreshLabel = computed(() => {
  if (!snapshot.value?.collectedAt) return '自动刷新';
  return `刷新于 ${new Date(snapshot.value.collectedAt * 1000).toLocaleTimeString()}`;
});

watch(
  () => ({
    hostId: workspaceStore.activeHost?.id || '',
    credentialVersion: workspaceStore.credentialVersion,
  }),
  () => restartMonitor(),
  { immediate: true },
);

onBeforeUnmount(() => {
  stopMonitor();
});

function restartMonitor() {
  stopMonitor();
  snapshot.value = null;
  previousSnapshot.value = null;
  loading.value = false;
  errorMessage.value = '';
  requestVersion += 1;

  const host = workspaceStore.activeHost;
  if (!host || !workspaceStore.hasCredential(host.id)) {
    return;
  }

  void loadSnapshot();
  timer = window.setInterval(() => {
    void loadSnapshot();
  }, 5000);
}

function stopMonitor() {
  if (timer) {
    window.clearInterval(timer);
    timer = undefined;
  }
}

async function loadSnapshot() {
  const version = ++requestVersion;
  const host = workspaceStore.activeHost;
  const credential = host ? workspaceStore.getCredential(host.id) : undefined;

  if (!host || !credential?.password) return;

  loading.value = true;
  errorMessage.value = '';

  try {
    const result = await invoke<MonitorSnapshot>('monitor_snapshot', {
      payload: {
        host: host.host,
        port: host.port,
        username: host.username,
        password: credential.password,
        privateKeyPath: null,
        passphrase: null,
      },
    });

    if (version !== requestVersion) return;
    if (workspaceStore.activeHost?.id !== host.id) return;
    if (!workspaceStore.hasCredential(host.id)) return;

    previousSnapshot.value = snapshot.value;
    snapshot.value = result;
  } catch {
    if (version !== requestVersion) return;
    errorMessage.value = '监控连接失败，请检查当前 SSH 认证或系统命令支持。';
  } finally {
    if (version === requestVersion) {
      loading.value = false;
    }
  }
}

function getNetworkRate(name: string, direction: 'rx' | 'tx') {
  const current = snapshot.value;
  const previous = previousSnapshot.value;
  if (!current || !previous) return '-';

  const currentNetwork = current.networks.find((item) => item.name === name);
  const previousNetwork = previous.networks.find((item) => item.name === name);
  if (!currentNetwork || !previousNetwork) return '-';

  const seconds = Math.max(1, current.collectedAt - previous.collectedAt);
  const currentBytes = direction === 'rx' ? currentNetwork.rxBytes : currentNetwork.txBytes;
  const previousBytes = direction === 'rx' ? previousNetwork.rxBytes : previousNetwork.txBytes;
  const delta = Math.max(0, currentBytes - previousBytes);

  return formatRate(delta / seconds);
}

function valueOrDash(value: string | undefined) {
  return value && value !== '-' ? value : '-';
}

function formatPercent(value: number | undefined) {
  if (value === undefined || Number.isNaN(value)) return '-';
  return `${value.toFixed(1)}%`;
}

function progressWidth(value: number | undefined) {
  if (value === undefined || Number.isNaN(value)) return '0%';
  return `${Math.min(100, Math.max(0, value))}%`;
}

function formatMemory(value: number | undefined) {
  if (!value) return '-';
  if (value < 1024) return `${value} MB`;
  return `${(value / 1024).toFixed(1)} GB`;
}

function formatRate(bytesPerSecond: number) {
  if (bytesPerSecond < 1024) return `${bytesPerSecond.toFixed(0)} B/s`;
  if (bytesPerSecond < 1024 * 1024) return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSecond / 1024 / 1024).toFixed(1)} MB/s`;
}
</script>

<style scoped>
.monitor-stack {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 10px;
}

.monitor-card,
.monitor-empty {
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 12px;
  background: linear-gradient(180deg, rgba(15, 23, 42, 0.82), rgba(8, 13, 23, 0.88));
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.2);
}

.monitor-card {
  padding: 12px;
}

.monitor-empty {
  display: grid;
  min-height: 180px;
  place-items: center;
  color: #94a3b8;
  padding: 18px;
  text-align: center;
}

.monitor-card__head,
.disk-item__head,
.network-item {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.monitor-card__head h2,
.monitor-card__head h3 {
  margin: 0;
  color: #e5e7eb;
  font-size: 15px;
}

.monitor-card__head span,
.monitor-card__head strong,
.disk-item__head strong {
  color: #cbd5e1;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}

.monitor-grid {
  display: grid;
  gap: 7px;
  margin: 12px 0 0;
}

.monitor-row {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 6px;
}

.monitor-row dt {
  color: #64748b;
  font-size: 12px;
}

.monitor-row dd,
.network-name,
.disk-item__head span {
  min-width: 0;
  overflow: hidden;
  margin: 0;
  color: #dbeafe;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.monitor-muted {
  overflow: hidden;
  margin: 8px 0 0;
  color: #94a3b8;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.monitor-progress {
  height: 5px;
  overflow: hidden;
  margin-top: 10px;
  border-radius: 999px;
  background: #1e293b;
}

.monitor-progress__bar {
  display: block;
  height: 100%;
  border-radius: inherit;
}

.monitor-progress__bar--cpu {
  background: linear-gradient(90deg, #3b82f6, #38bdf8);
}

.monitor-progress__bar--memory {
  background: linear-gradient(90deg, #22c55e, #38bdf8);
}

.monitor-progress__bar--swap {
  background: linear-gradient(90deg, #a78bfa, #60a5fa);
}

.monitor-progress__bar--disk {
  background: linear-gradient(90deg, #22c55e, #f59e0b);
}

.monitor-list {
  display: grid;
  gap: 10px;
  margin-top: 10px;
}

.network-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 72px 72px;
  align-items: center;
  border-top: 1px solid rgba(148, 163, 184, 0.1);
  padding-top: 8px;
}

.network-rate {
  font-size: 11px;
  text-align: right;
  white-space: nowrap;
}

.network-rate--down {
  color: #22c55e;
}

.network-rate--up {
  color: #60a5fa;
}

.disk-item {
  min-width: 0;
  border-top: 1px solid rgba(148, 163, 184, 0.1);
  padding-top: 8px;
}
</style>
