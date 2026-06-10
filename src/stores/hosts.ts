import { computed, ref } from 'vue';
import { defineStore } from 'pinia';

export interface HostProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  group: string;
  remark?: string;
  lastConnectedAt?: number;
  createdAt: number;
  updatedAt: number;
}

export interface HostProfileInput {
  id?: string;
  name: string;
  host: string;
  port: number;
  username: string;
  group?: string;
  remark?: string;
}

const STORAGE_KEY = 'lite-shell:hosts:v1';
const DEFAULT_GROUP = '默认分组';

function createId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function normalizeGroup(group: string | undefined) {
  return group?.trim() || DEFAULT_GROUP;
}

function normalizeHost(input: HostProfileInput, now = Date.now()): HostProfile {
  const name = input.name.trim() || `${input.username.trim()}@${input.host.trim()}`;

  return {
    id: input.id || createId(),
    name,
    host: input.host.trim(),
    port: Number(input.port) || 22,
    username: input.username.trim(),
    group: normalizeGroup(input.group),
    remark: input.remark?.trim() || undefined,
    createdAt: now,
    updatedAt: now,
  };
}

function loadHosts(): HostProfile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];

    const parsed = JSON.parse(raw) as Partial<HostProfile>[];
    if (!Array.isArray(parsed)) return [];

    return parsed
      .filter((item) => item.id && item.host && item.username && Number.isFinite(item.port))
      .map((item) => ({
        id: String(item.id),
        name: String(item.name || `${item.username}@${item.host}`),
        host: String(item.host),
        port: Number(item.port) || 22,
        username: String(item.username),
        group: normalizeGroup(item.group),
        remark: typeof item.remark === 'string' && item.remark.trim() ? item.remark.trim() : undefined,
        lastConnectedAt: Number.isFinite(item.lastConnectedAt) ? Number(item.lastConnectedAt) : undefined,
        createdAt: Number.isFinite(item.createdAt) ? Number(item.createdAt) : Date.now(),
        updatedAt: Number.isFinite(item.updatedAt) ? Number(item.updatedAt) : Date.now(),
      }));
  } catch {
    return [];
  }
}

function saveHosts(hosts: HostProfile[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(hosts));
}

export const useHostStore = defineStore('hosts', () => {
  const hosts = ref<HostProfile[]>(loadHosts());

  const sortedHosts = computed(() =>
    [...hosts.value].sort((left, right) => right.updatedAt - left.updatedAt),
  );
  const recentHosts = computed(() =>
    [...hosts.value]
      .filter((host) => host.lastConnectedAt)
      .sort((left, right) => (right.lastConnectedAt || 0) - (left.lastConnectedAt || 0))
      .slice(0, 8),
  );
  const groups = computed(() => {
    const names = hosts.value.map((host) => normalizeGroup(host.group));
    return [DEFAULT_GROUP, ...names]
      .filter((name, index, array) => array.indexOf(name) === index)
      .sort((left, right) => {
        if (left === DEFAULT_GROUP) return -1;
        if (right === DEFAULT_GROUP) return 1;
        return left.localeCompare(right, 'zh-CN');
      });
  });

  function upsertHost(input: HostProfileInput) {
    const now = Date.now();
    const currentIndex = input.id
      ? hosts.value.findIndex((item) => item.id === input.id)
      : -1;

    if (currentIndex >= 0) {
      const current = hosts.value[currentIndex];
      hosts.value[currentIndex] = {
        ...normalizeHost(input, now),
        id: current.id,
        lastConnectedAt: current.lastConnectedAt,
        createdAt: current.createdAt,
        updatedAt: now,
      };
    } else {
      hosts.value.unshift(normalizeHost(input, now));
    }

    saveHosts(hosts.value);
    return input.id ? hosts.value.find((item) => item.id === input.id) : hosts.value[0];
  }

  function removeHost(id: string) {
    hosts.value = hosts.value.filter((item) => item.id !== id);
    saveHosts(hosts.value);
  }

  function touchHost(id: string) {
    const host = hosts.value.find((item) => item.id === id);
    if (!host) return;

    const now = Date.now();
    host.updatedAt = now;
    host.lastConnectedAt = now;
    saveHosts(hosts.value);
  }

  return {
    hosts,
    sortedHosts,
    recentHosts,
    groups,
    upsertHost,
    removeHost,
    touchHost,
  };
});
