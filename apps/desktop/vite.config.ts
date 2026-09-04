import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

const backendUrl = process.env.BACKEND_URL ?? 'http://localhost:8080';
const isTauri = !!process.env.TAURI_ENV_PLATFORM;

// Tauri dev mode: use fixed port, enable strictPort so tauri.conf.json devUrl works
const serverConfig = isTauri
  ? {
      port: 5173,
      strictPort: true,
      // Tauri 模式下不 proxy — 前端通过 IPC 调 Rust，不走 HTTP
    }
  : {
      proxy: {
        '/healthz': { target: backendUrl, changeOrigin: true },
        '/execute': { target: backendUrl, changeOrigin: true },
        '/sse/execute': { target: backendUrl, changeOrigin: true, ws: false },
        '/openapi.json': { target: backendUrl, changeOrigin: true },
        '/docs': { target: backendUrl, changeOrigin: true },
      },
    };

export default defineConfig({
  plugins: [vue()],
  // Tauri 使用相对路径加载资源
  base: isTauri ? './' : '/',
  server: serverConfig,
  // 防止 Vite 缓存干扰 Tauri 的热重载
  clearScreen: false,
  test: {
    environment: 'happy-dom',
    globals: true,
    include: ['src/**/*.test.ts'],
  },
});
