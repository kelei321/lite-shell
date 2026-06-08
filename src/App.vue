<template>
  <main class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand__logo">L</div>
        <div>
          <h1>LiteShell</h1>
          <p>轻量 SSH / SFTP 客户端</p>
        </div>
      </div>

      <nav class="nav-list">
        <button
          class="nav-item"
          :class="{ 'nav-item--active': activeView === 'terminal' }"
          type="button"
          @click="activeView = 'terminal'"
        >
          终端
        </button>
        <button
          class="nav-item"
          :class="{ 'nav-item--active': activeView === 'sftp' }"
          type="button"
          @click="activeView = 'sftp'"
        >
          文件
        </button>
        <button class="nav-item" type="button" disabled>监控</button>
        <button class="nav-item" type="button" disabled>快捷命令</button>
      </nav>
    </aside>

    <TerminalView v-show="activeView === 'terminal'" />
    <SftpView v-show="activeView === 'sftp'" />
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue';

import SftpView from './pages/sftp/SftpView.vue';
import TerminalView from './pages/terminal/TerminalView.vue';

const activeView = ref<'terminal' | 'sftp'>('terminal');
</script>
