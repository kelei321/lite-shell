import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const file = resolve('src/pages/terminal/TerminalView.vue');
let code = readFileSync(file, 'utf8');

if (!code.includes('lastCols?: number;')) {
  code = code.replace(
    '  fitAddon?: FitAddon;\n}',
    '  fitAddon?: FitAddon;\n  lastCols?: number;\n  lastRows?: number;\n}',
  );
}

code = code.replace(
  'resizeObserver = new ResizeObserver(() => resizeActiveTab());',
  'resizeObserver = new ResizeObserver(() => scheduleActiveTabResize());',
);

code = code.replace(
  `  if (tab.terminal) {
    resizeActiveTab();
    return;
  }`,
  `  if (tab.terminal) {
    return;
  }`,
);

code = code.replace(
  '    fitAddon.fit();\n\n    const sessionId = await invoke<string>(\'ssh_connect\'',
  '    fitVisibleTab(tab);\n\n    const sessionId = await invoke<string>(\'ssh_connect\'',
);

code = code.replace(
  '  resizeObserver?.observe(hostElement);\n  fitAddon.fit();',
  '  resizeObserver?.observe(hostElement);\n  scheduleActiveTabResize();',
);

code = code.replace(
  `function activateTab(tabId: string) {
  activeTabId.value = tabId;
  void nextTick(() => resizeActiveTab());
}`,
  `function activateTab(tabId: string) {
  activeTabId.value = tabId;
  scheduleActiveTabResize();
}`,
);

code = code.replace(
  `function activateTab(tabId: string) {
  activeTabId.value = tabId;
  void nextTick(() => {
    resizeActiveTab();
    activeTab.value?.terminal?.focus();
  });
}`,
  `function activateTab(tabId: string) {
  activeTabId.value = tabId;
  scheduleActiveTabResize();
}`,
);

code = code.replace(
  `  if (activeTabId.value === tabId) {
    activeTabId.value = tabs.value[Math.max(0, index - 1)]?.id || '';
  }
}`,
  `  if (activeTabId.value === tabId) {
    activeTabId.value = tabs.value[Math.max(0, index - 1)]?.id || '';
    scheduleActiveTabResize();
  }
}`,
);

code = code.replace(
  /function resizeActiveTab\(\) \{[\s\S]*?\n\}\n\nfunction createId\(\)/,
  `function scheduleActiveTabResize() {
  const tabId = activeTabId.value;

  void nextTick(() => {
    requestAnimationFrame(() => {
      if (activeTabId.value !== tabId) return;
      resizeActiveTab();
      activeTab.value?.terminal?.focus();
    });
  });
}

function resizeActiveTab() {
  const tab = activeTab.value;
  if (!tab?.terminal || !tab.fitAddon) return;

  fitVisibleTab(tab);
}

function fitVisibleTab(tab: TerminalTab) {
  if (!tab.terminal || !tab.fitAddon) return;

  const hostElement = terminalHosts.get(tab.id);
  if (!hostElement) return;

  const rect = hostElement.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;

  tab.fitAddon.fit();

  const { cols, rows } = tab.terminal;
  if (!tab.sessionId || cols <= 0 || rows <= 0) return;
  if (tab.lastCols === cols && tab.lastRows === rows) return;

  tab.lastCols = cols;
  tab.lastRows = rows;

  void invoke('ssh_resize', {
    id: tab.sessionId,
    cols,
    rows,
  });
}

function createId()`,
);

writeFileSync(file, code);
