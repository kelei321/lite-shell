<template>
  <section class="terminal-layout">
    <header class="toolbar">
      <div>
        <h2>SSH 终端</h2>
        <p>第一版密码不落盘，关闭连接后会释放 SSH session。</p>
      </div>
      <div class="status" :class="{ 'status--online': isConnected }">
        {{ isConnected ? '已连接' : '未连接' }}
      </div>
    </header>

    <div class="content-grid">
      <form class="connect-card" @submit.prevent="connect">
        <label>
          <span>主机</span>
          <input v-model.trim="form.host" autocomplete="off" placeholder="127.0.0.1" />
        </label>

        <label>
          <span>端口</span>
          <input v-model.number="form.port" min="1" max="65535" type="number" />
        </label>

        <label>
          <span>用户名</span>
          <input v-model.trim="form.username" autocomplete="username" placeholder="root" />
        </label>

        <label>
          <span>密码</span>
          <input v-model="form.password" autocomplete="current-password" type="password" />
        </label>

        <div class="action-row">
          <button class="primary-button" :disabled="connecting || isConnected" type="submit">
            {{ connecting ? '连接中...' : '连接' }}
          </button>
          <button class="ghost-button" :disabled="!isConnected" type="button" @click="close">
            断开
          </button>
        </div>

        <section class="quick-card">
          <h3>快捷命令</h3>
          <button
            v-for="command in quickCommands"
            :key="command"
            class="command-button"
            :disabled="!isConnected"
            type="button"
            @click="sendCommand(command)"
          >
            {{ command }}
          </button>
        </section>
      </form>

      <div class="terminal-card">
        <div ref="terminalEl" class="terminal-host"></div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';

import '@xterm/xterm/css/xterm.css';

interface SshDataPayload {
  id: string;
  data: string;
}

const form = reactive({
  host: '127.0.0.1',
  port: 22,
  username: 'root',
  password: '',
});

const quickCommands = ['pwd', 'ls -la', 'df -h', 'free -m', 'top'];
const terminalEl = ref<HTMLDivElement>();
const sessionId = ref('');
const connecting = ref(false);
const isConnected = computed(() => Boolean(sessionId.value));

let terminal: Terminal | undefined;
let fitAddon: FitAddon | undefined;
let resizeObserver: ResizeObserver | undefined;
let unlisten: UnlistenFn | undefined;

onMounted(async () => {
  terminal = new Terminal({
    cursorBlink: true,
    convertEol: true,
    fontFamily: 'Consolas, "JetBrains Mono", "Noto Sans Mono CJK SC", monospace',
    fontSize: 14,
    scrollback: 6000,
    theme: {
      background: '#020617',
      foreground: '#e5e7eb',
    },
  });

  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(terminalEl.value!);

  await nextTick();
  fitAddon.fit();
  terminal.writeln('LiteShell ready. 输入主机信息后点击连接。');

  terminal.onData((data) => {
    if (!sessionId.value) return;
    void invoke('ssh_write', { id: sessionId.value, data });
  });

  unlisten = await listen<SshDataPayload>('ssh:data', (event) => {
    if (!terminal || event.payload.id !== sessionId.value) return;
    terminal.write(event.payload.data);
  });

  resizeObserver = new ResizeObserver(() => resizeTerminal());
  resizeObserver.observe(terminalEl.value!);
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  unlisten?.();
  terminal?.dispose();

  if (sessionId.value) {
    void invoke('ssh_close', { id: sessionId.value });
  }
});

async function connect() {
  if (!terminal || !fitAddon) return;

  connecting.value = true;
  terminal.clear();
  terminal.writeln(`Connecting to ${form.username}@${form.host}:${form.port} ...`);

  try {
    fitAddon.fit();

    const id = await invoke<string>('ssh_connect', {
      payload: {
        host: form.host,
        port: form.port,
        username: form.username,
        password: form.password || null,
        privateKeyPath: null,
        passphrase: null,
        cols: terminal.cols,
        rows: terminal.rows,
      },
    });

    sessionId.value = id;
    terminal.writeln('Connected.');
  } catch (error) {
    terminal.writeln(`Connect failed: ${String(error)}`);
  } finally {
    connecting.value = false;
  }
}

async function close() {
  if (!sessionId.value || !terminal) return;

  const id = sessionId.value;
  sessionId.value = '';
  await invoke('ssh_close', { id });
  terminal.writeln('\r\nDisconnected.');
}

function sendCommand(command: string) {
  if (!sessionId.value) return;
  void invoke('ssh_write', {
    id: sessionId.value,
    data: `${command}\n`,
  });
}

function resizeTerminal() {
  if (!terminal || !fitAddon) return;

  fitAddon.fit();

  if (!sessionId.value) return;

  void invoke('ssh_resize', {
    id: sessionId.value,
    cols: terminal.cols,
    rows: terminal.rows,
  });
}
</script>

<style scoped>
.terminal-layout {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  padding: 18px;
  gap: 16px;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 64px;
  border: 1px solid #1e293b;
  border-radius: 16px;
  background: #0f172a;
  padding: 14px 18px;
}

.toolbar h2 {
  margin: 0;
  font-size: 20px;
}

.toolbar p {
  margin: 6px 0 0;
  color: #94a3b8;
  font-size: 13px;
}

.status {
  border-radius: 999px;
  background: #334155;
  color: #cbd5e1;
  padding: 6px 12px;
  font-size: 13px;
}

.status--online {
  background: rgba(34, 197, 94, 0.14);
  color: #86efac;
}

.content-grid {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  min-height: 0;
  flex: 1;
  gap: 16px;
}

.connect-card,
.terminal-card {
  border: 1px solid #1e293b;
  border-radius: 16px;
  background: #0f172a;
}

.connect-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.connect-card label {
  display: grid;
  gap: 6px;
}

.connect-card span {
  color: #94a3b8;
  font-size: 12px;
}

.connect-card input {
  width: 100%;
  height: 36px;
  border: 1px solid #334155;
  border-radius: 10px;
  outline: none;
  background: #020617;
  color: #e5e7eb;
  padding: 0 10px;
}

.connect-card input:focus {
  border-color: #2563eb;
}

.action-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-top: 4px;
}

.primary-button,
.ghost-button,
.command-button {
  height: 36px;
  border-radius: 10px;
  color: #fff;
  cursor: pointer;
}

.primary-button {
  background: #2563eb;
}

.ghost-button,
.command-button {
  border: 1px solid #334155;
  background: #1e293b;
}

.primary-button:disabled,
.ghost-button:disabled,
.command-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.quick-card {
  display: grid;
  gap: 8px;
  margin-top: 10px;
}

.quick-card h3 {
  margin: 0 0 4px;
  color: #cbd5e1;
  font-size: 14px;
}

.command-button {
  text-align: left;
  padding: 0 10px;
}

.terminal-card {
  min-width: 0;
  min-height: 0;
  padding: 10px;
}

.terminal-host {
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 12px;
  background: #020617;
}
</style>
