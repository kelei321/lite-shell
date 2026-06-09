<template>
  <div class="monitor-stack">
    <div v-if="statusMessage" class="monitor-empty">{{ statusMessage }}</div>

    <template v-else>
      <section class="compact-section compact-section--plain">
        <div class="host-line">
          <span>主机</span>
          <strong>{{ valueOrDash(snapshot?.hostname) }}</strong>
        </div>
        <div class="host-line">
          <span>地址</span>
          <strong>{{ activeHostLabel }}</strong>
        </div>
        <div class="host-line">
          <span>系统</span>
          <strong>{{ valueOrDash(snapshot?.os) }}</strong>
        </div>
        <div class="host-line">
          <span>内核</span>
          <strong>{{ valueOrDash(snapshot?.kernel) }}</strong>
        </div>
      </section>

      <section class="compact-section compact-section--plain">
        <div class="metric-line metric-line--text">
          <span>运行</span>
          <strong>{{ valueOrDash(snapshot?.uptime) }}</strong>
        </div>
        <div class="metric-line metric-line--text">
          <span>负载</span>
          <strong>-</strong>
        </div>
        <div class="metric-line">
          <span>CPU</span>
          <div class="thin-progress">
            <i class="thin-progress__bar thin-progress__bar--cpu" :style="{ width: progressWidth(snapshot?.cpuUsage) }"></i>
          </div>
          <strong>{{ formatPercent(snapshot?.cpuUsage) }}</strong>
        </div>
        <div class="metric-line">
          <span>内存</span>
          <div class="thin-progress">
            <i class="thin-progress__bar thin-progress__bar--memory" :style="{ width: progressWidth(snapshot?.memory.usagePercent) }"></i>
          </div>
          <strong>{{ formatMemoryPair(snapshot?.memory.usedMb, snapshot?.memory.totalMb) }}</strong>
        </div>
        <div class="metric-line">
          <span>交换</span>
          <div class="thin-progress">
            <i class="thin-progress__bar thin-progress__bar--swap" :style="{ width: progressWidth(snapshot?.swap.usagePercent) }"></i>
          </div>
          <strong>{{ formatMemoryPair(snapshot?.swap.usedMb, snapshot?.swap.totalMb) }}</strong>
        </div>
      </section>

      <section class="compact-section">
        <div class="section-title">
          <span>网络</span>
          <strong>{{ refreshLabel }}</strong>
        </div>
        <div v-if="snapshot?.networks.length" class="network-table">
          <div v-for="network in snapshot.networks" :key="network.name" class="network-row">
            <span class="cell-ellipsis">{{ network.name }}</span>
            <strong class="rate-down">↓ {{ getNetworkRate(network.name, 'rx') }}</strong>
            <strong class="rate-up">↑ {{ getNetworkRate(network.name, 'tx') }}</strong>
          </div>
        </div>
        <p v-else class="compact-muted">-</p>
      </section>

      <section class="compact-section compact-section--table">
        <div class="disk-table">
          <div class="disk-table__head">
            <span>路径</span>
            <span>可用/大小</span>
          </div>
          <div v-if="snapshot?.disks.length" class="disk-table__body">
            <div v-for="disk in snapshot.disks" :key="`${disk.filesystem}-${disk.mount}`" class="disk-row">
              <span class="cell-ellipsis" :title="disk.mount">{{ disk.mount || '-' }}</span>
              <strong>{{ formatDiskSize(disk) }}</strong>
              <div class="disk-progress" :title="formatPercent(disk.usagePercent)">
                <i :style="{ width: progressWidth(disk.usagePercent) }"></i>
              </div>
            </div>
          </div>
          <p v-else class="compact-muted">-</p>
        </div>
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

import { useWorkspaceStore } from '@/stores/workspace';
import type { DiskSnapshot, MonitorSnapshot } from '@/types/monitor';

const workspaceStore = useWorkspaceStore();

const snapshot = ref<MonitorSnapshot | null>(null);
const previousSnapshot = ref<MonitorSnapshot | null>(null);
const loading = ref(false);
const errorMessage = ref('');

let timer: number | undefined;
let requestVersion = 0;
let inFlight = false;

const activeHost = computed(() => workspaceStore.activeHost);
const activeHostLabel = computed(() => {
  const host = activeHost.value;
  if (!host) return '-';
  return `${host.username}@${host.host}`;
});
const statusMessage = computed(() => {
  if (!activeHost.value) return '请先连接 SSH 主机';
  if (!workspaceStore.hasCredential(activeHost.value.id)) return '当前主机未缓存认证';
  if (loading.value && !snapshot.value) return '监控加载中...';
  if (errorMessage.value) return errorMessage.value;
  return '';
});
const refreshLabel = computed(() => {
  if (!snapshot.value?.collectedAt) return '自动刷新';
  return new Date(snapshot.value.collectedAt * 1000).toLocaleTimeString();
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
  requestVersion += 1;
  inFlight = false;
});

function restartMonitor() {
  stopMonitor();
  snapshot.value = null;
  previousSnapshot.value = null;
  loading.value = false;
  errorMessage.value = '';
  inFlight = false;
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
  if (inFlight) return;

  const version = ++requestVersion;
  const host = workspaceStore.activeHost;
  const credential = host ? workspaceStore.getCredential(host.id) : undefined;

  if (!host || !credential?.password) return;

  inFlight = true;
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
      inFlight = false;
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
  if (value < 1024) return `${value}M`;
  return `${(value / 1024).toFixed(1)}G`;
}

function formatMemoryPair(used: number | undefined, total: number | undefined) {
  if (!total) return '-';
  return `${formatMemory(used)} / ${formatMemory(total)}`;
}

function formatDiskSize(disk: DiskSnapshot) {
  return `${disk.available || '-'} / ${disk.total || '-'}`;
}

function formatRate(bytesPerSecond: number) {
  if (bytesPerSecond < 1024) return `${bytesPerSecond.toFixed(0)}B/s`;
  if (bytesPerSecond < 1024 * 1024) return `${(bytesPerSecond / 1024).toFixed(1)}K/s`;
  return `${(bytesPerSecond / 1024 / 1024).toFixed(1)}M/s`;
}
</script>

<style scoped>
.monitor-stack {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 6px;
  color: #dbeafe;
  font-size: 12px;
}

.compact-section,
.monitor-empty {
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 6px;
  background: rgba(2, 6, 23, 0.34);
}

.compact-section {
  overflow: hidden;
}

.compact-section--plain {
  display: grid;
  gap: 2px;
  border-color: transparent;
  background: transparent;
}

.monitor-empty {
  display: grid;
  min-height: 118px;
  place-items: center;
  color: #94a3b8;
  padding: 12px;
  text-align: center;
}

.host-line,
.metric-line,
.section-title,
.network-row,
.disk-table__head,
.disk-row {
  display: grid;
  min-width: 0;
  align-items: center;
  gap: 6px;
}

.host-line,
.metric-line--text {
  grid-template-columns: 34px minmax(0, 1fr);
}

.host-line span,
.metric-line span {
  color: #cbd5e1;
  white-space: nowrap;
}

.host-line strong,
.metric-line strong,
.section-title strong,
.network-row strong,
.disk-row strong {
  min-width: 0;
  overflow: hidden;
  color: #e5e7eb;
  font-size: 11px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metric-line {
  grid-template-columns: 34px minmax(0, 1fr) 78px;
  min-height: 18px;
}

.thin-progress,
.disk-progress {
  height: 13px;
  overflow: hidden;
  border: 1px solid rgba(148, 163, 184, 0.32);
  background: rgba(15, 23, 42, 0.84);
}

.thin-progress__bar,
.disk-progress i {
  display: block;
  height: 100%;
}

.thin-progress__bar--cpu {
  background: rgba(134, 239, 172, 0.72);
}

.thin-progress__bar--memory {
  background: rgba(251, 191, 36, 0.44);
}

.thin-progress__bar--swap {
  background: rgba(96, 165, 250, 0.44);
}

.section-title {
  grid-template-columns: minmax(0, 1fr) auto;
  height: 22px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.18);
  background: rgba(30, 41, 59, 0.7);
  color: #e5e7eb;
  padding: 0 6px;
}

.section-title span,
.disk-table__head span {
  font-weight: 600;
}

.network-table {
  display: grid;
}

.network-row {
  grid-template-columns: minmax(0, 1fr) 58px 58px;
  min-height: 22px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.08);
  padding: 0 6px;
}

.network-row:last-child {
  border-bottom: 0;
}

.rate-down {
  color: #86efac;
  text-align: right;
}

.rate-up {
  color: #fca5a5;
  text-align: right;
}

.disk-table {
  display: grid;
  min-width: 0;
}

.disk-table__head {
  grid-template-columns: minmax(0, 1fr) 82px;
  height: 24px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.18);
  background: rgba(30, 41, 59, 0.7);
  color: #e5e7eb;
  padding: 0 6px;
}

.disk-table__body {
  display: grid;
}

.disk-row {
  grid-template-columns: minmax(0, 1fr) 82px;
  min-height: 22px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.06);
  padding: 2px 6px 3px;
}

.disk-row:nth-child(even) {
  background: rgba(148, 163, 184, 0.04);
}

.disk-row strong {
  text-align: right;
}

.disk-progress {
  grid-column: 1 / -1;
  height: 3px;
  border: 0;
  background: rgba(30, 41, 59, 0.8);
}

.disk-progress i {
  background: linear-gradient(90deg, #22c55e, #f59e0b);
}

.cell-ellipsis,
.compact-muted {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.compact-muted {
  margin: 0;
  color: #94a3b8;
  padding: 6px;
}
</style>
