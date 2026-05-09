import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
var host = process.env.TAURI_DEV_HOST;
// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react()],
    resolve: {
        alias: {
            "@": path.resolve(__dirname, "./src"),
        },
    },
    // 1. 阻止 Vite 隐藏来自 Rust 的错误
    clearScreen: false,
    // 2. Tauri 期望的固定端口；移动端开发需要 host 暴露给设备
    server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
            ? {
                protocol: "ws",
                host: host,
                port: 1421,
            }
            : undefined,
        watch: {
            ignored: ["**/src-tauri/**"],
        },
    },
});
