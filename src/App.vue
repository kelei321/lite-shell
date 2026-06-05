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
        <button class="nav-item nav-item--active" type="button">终端</button>
        <button class="nav-item" type="button" disabled>SFTP</button>
        <button class="nav-item" type="button" disabled>监控</button>
        <button class="nav-item" type="button" disabled>快捷命令</button>
      </nav>
    </aside>

    <TerminalView />
  </main>
</template>

<script setup lang="ts">
import TerminalView from './pages/terminal/TerminalView.vue';
</script>
