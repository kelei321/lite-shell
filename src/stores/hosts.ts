import { computed, ref } from 'vue';
import { defineStore } from 'pinia';

export interface HostProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  createdAt: number;
  updatedAt: number;
}

export interface HostProfileInput {
  id?: string;
  name: string;
  host: string;
  port: number;
  username: string;
}

const STORAGE_KEY = 'lite-shell:hosts:v1';

function createId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function normalizeHost(input: HostProfileInput, now = Date.now()): HostProfile {
  const name = input.name.trim() || `${input.username.trim()}@${input.host.trim()}`;

  return {
    id: input.id || createId(),
    name,
    host: input.host.trim(),
    port: Number(input.port) || 22,
    username: input.username.trim(),
    createdAt: now,
    updatedAt: now,
  };
}

function loadHosts(): HostProfile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];

    const parsed = JSON.parse(raw) as HostProfile[];
    if (!Array.isArray(parsed)) return [];

    return parsed.filter(
      (item) => item.id && item.host && item.username && Number.isFinite(item.port),
    );
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

    host.updatedAt = Date.now();
    saveHosts(hosts.value);
  }

  return {
    hosts,
    sortedHosts,
    upsertHost,
    removeHost,
    touchHost,
  };
});
