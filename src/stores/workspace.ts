import { computed, ref, shallowRef } from 'vue';
import { defineStore } from 'pinia';

import type { HostProfile } from './hosts';

export type WorkspaceHost = Pick<HostProfile, 'id' | 'name' | 'host' | 'port' | 'username'>;

export interface WorkspaceCredential {
  hostId: string;
  host: string;
  port: number;
  username: string;
  password?: string;
  privateKeyPath?: string;
  passphrase?: string;
  source: 'ssh';
  createdAt: number;
}

export const useWorkspaceStore = defineStore('workspace', () => {
  const activeHost = ref<WorkspaceHost | null>(null);
  const credentialsByHostId = shallowRef<Record<string, WorkspaceCredential>>({});

  const activeHostLabel = computed(() => activeHost.value?.host || '未连接');
  const hasActiveHost = computed(() => Boolean(activeHost.value));
  const activeHostHasCredential = computed(() =>
    activeHost.value ? hasCredential(activeHost.value.id) : false,
  );

  function setActiveHost(host: WorkspaceHost) {
    activeHost.value = { ...host };
  }

  function clearActiveHost(hostId?: string) {
    if (!hostId || activeHost.value?.id === hostId) {
      activeHost.value = null;
    }
  }

  function setCredential(credential: WorkspaceCredential) {
    credentialsByHostId.value = {
      ...credentialsByHostId.value,
      [credential.hostId]: { ...credential },
    };
  }

  function getCredential(hostId: string) {
    return credentialsByHostId.value[hostId];
  }

  function hasCredential(hostId: string) {
    return Boolean(credentialsByHostId.value[hostId]);
  }

  function clearCredential(hostId: string) {
    if (!credentialsByHostId.value[hostId]) return;

    const next = { ...credentialsByHostId.value };
    delete next[hostId];
    credentialsByHostId.value = next;
  }

  function clearAllCredentials() {
    credentialsByHostId.value = {};
  }

  return {
    activeHost,
    activeHostLabel,
    activeHostHasCredential,
    hasActiveHost,
    setActiveHost,
    clearActiveHost,
    setCredential,
    getCredential,
    hasCredential,
    clearCredential,
    clearAllCredentials,
  };
});
