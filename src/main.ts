import { createApp } from "vue";
import App from "./App.vue";
import { registerSftpPathShortcuts } from "./sftp/path-toolbar-shortcuts";
import "./styles.css";
import "./sftp-polish.css";

createApp(App).mount("#root");

const stopSftpPathShortcuts = registerSftpPathShortcuts();
window.addEventListener("beforeunload", stopSftpPathShortcuts, { once: true });
