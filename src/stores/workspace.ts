import { computed, ref } from 'vue';
import { defineStore } from 'pinia';

import type { HostProfile } from './hosts';

export type WorkspaceHost = Pick<HostProfile, 'id' | 'name' | 'host' | 'port' | 'username'>;

export const useWorkspaceStore = defineStore('workspace', () => {
  const activeHost = ref<WorkspaceHost | null>(null);

  const activeHostLabel = computed(() => activeHost.value?.host || '未连接');
  const hasActiveHost = computed(() => Boolean(activeHost.value));

  function setActiveHost(host: WorkspaceHost) {
    activeHost.value = { ...host };
  }

  function clearActiveHost(hostId?: string) {
    if (!hostId || activeHost.value?.id === hostId) {
      activeHost.value = null;
    }
  }

  return {
    activeHost,
    activeHostLabel,
    hasActiveHost,
    setActiveHost,
    clearActiveHost,
  };
});
