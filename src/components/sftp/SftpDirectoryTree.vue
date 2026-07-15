<script setup lang="ts">
import { computed } from "vue";
import {
  IconAlertCircle,
  IconChevronDown,
  IconChevronRight,
  IconFolder,
  IconRefresh,
} from "@tabler/icons-vue";
import {
  visibleDirectoryTreeNodes,
  type SftpDirectoryTreeState,
} from "../../sftp/directory-tree-state";

const props = defineProps<{
  state: SftpDirectoryTreeState;
  connected: boolean;
}>();

const emit = defineEmits<{
  open: [path: string];
  toggle: [path: string];
  refresh: [path: string];
}>();

const rows = computed(() => visibleDirectoryTreeNodes(props.state));
const refreshPath = computed(() => props.state.selectedPath || props.state.rootPath);
</script>

<template>
  <section class="sftp-directory-tree" aria-label="远程目录树">
    <header class="sftp-directory-tree-header">
      <strong>远程目录</strong>
      <button
        class="icon-button"
        :disabled="!connected"
        :aria-label="`刷新目录树 ${refreshPath}`"
        @click="emit('refresh', refreshPath)"
      >
        <IconRefresh :size="15" />
      </button>
    </header>

    <div v-if="connected" class="sftp-directory-tree-scroll" role="tree">
      <div
        v-for="row in rows"
        :key="row.node.path"
        class="sftp-directory-tree-row"
        :class="{ selected: row.node.path === state.selectedPath, error: Boolean(row.node.error) }"
        :style="{ '--tree-depth': row.depth }"
        role="treeitem"
        :aria-level="row.depth + 1"
        :aria-expanded="row.node.loaded && !row.node.children.length ? undefined : row.node.expanded"
        :aria-selected="row.node.path === state.selectedPath"
        :title="row.node.error || row.node.path"
      >
        <button
          v-if="!row.node.loaded || row.node.children.length || row.node.error"
          class="sftp-directory-tree-toggle"
          :aria-label="row.node.expanded ? `折叠 ${row.node.name}` : `展开 ${row.node.name}`"
          :disabled="row.node.loading"
          @click.stop="emit('toggle', row.node.path)"
        >
          <IconChevronDown v-if="row.node.expanded" :size="14" />
          <IconChevronRight v-else :size="14" />
        </button>
        <span v-else class="sftp-directory-tree-toggle-spacer"></span>

        <button
          class="sftp-directory-tree-label"
          @click="emit('open', row.node.path)"
          @dblclick="emit('toggle', row.node.path)"
        >
          <IconFolder :size="16" />
          <span>{{ row.node.name }}</span>
        </button>

        <IconRefresh v-if="row.node.loading" class="sftp-directory-tree-loading" :size="13" />
        <button
          v-else-if="row.node.error"
          class="sftp-directory-tree-error"
          :aria-label="`重试读取 ${row.node.name}`"
          @click.stop="emit('refresh', row.node.path)"
        >
          <IconAlertCircle :size="14" />
        </button>
      </div>
    </div>

    <div v-else class="sftp-directory-tree-empty">
      <IconFolder :size="30" />
      <strong>尚未连接服务器</strong>
      <span>连接后按需加载远程目录</span>
    </div>
  </section>
</template>
