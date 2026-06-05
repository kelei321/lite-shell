import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const file = resolve('src/pages/terminal/TerminalView.vue');
let code = readFileSync(file, 'utf8');

code = code.replace(
  "    terminal.writeln('Connected.');\n  } catch (error) {",
  "    terminal.writeln('Connected.');\n    terminal.focus();\n  } catch (error) {",
);

code = code.replace(
  "  terminal.open(hostElement);\n  terminal.writeln('LiteShell ready.');\n\n  terminal.onData((data) => {",
  "  terminal.open(hostElement);\n  terminal.writeln('LiteShell ready.');\n  hostElement.addEventListener('pointerdown', () => terminal.focus());\n\n  terminal.onData((data) => {",
);

code = code.replace(
  "function activateTab(tabId: string) {\n  activeTabId.value = tabId;\n  void nextTick(() => resizeActiveTab());\n}",
  "function activateTab(tabId: string) {\n  activeTabId.value = tabId;\n  void nextTick(() => {\n    resizeActiveTab();\n    activeTab.value?.terminal?.focus();\n  });\n}",
);

code = code.replace(
  "  resizeObserver?.observe(hostElement);\n  fitAddon.fit();\n}",
  "  resizeObserver?.observe(hostElement);\n  fitAddon.fit();\n\n  if (tab.id === activeTabId.value) {\n    terminal.focus();\n  }\n}",
);

writeFileSync(file, code);
