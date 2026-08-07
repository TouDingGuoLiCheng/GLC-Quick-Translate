import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [vue()],
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        bubble: resolve(__dirname, "bubble.html"),
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1422,
    strictPort: true,
    // Windows 上 localhost 常解析到 ::1，Vite 只听 IPv6 时 Tauri 连 127.0.0.1 会一直 Waiting
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1423,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
