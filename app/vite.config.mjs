import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  optimizeDeps: {
    include: ["vue"],
  },
  server: {
    host: "0.0.0.0",
    allowedHosts: ["terminal.local"],
    watch: {
      ignored: ["**/src-tauri/target/**"],
    },
    warmup: {
      clientFiles: ["./src/main.ts", "./src/App.vue"],
    },
  },
  plugins: [vue()],
});
