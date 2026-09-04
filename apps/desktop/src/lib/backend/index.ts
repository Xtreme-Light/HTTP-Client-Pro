import type { BackendAdapter } from '../../types/http';
import { HttpAdapter } from './http';
import { TauriAdapter } from './tauri';

export { HttpAdapter, TauriAdapter };
export type { HttpAdapterOptions } from './http';
export type { TauriAdapterOptions } from './tauri';

/**
 * 检测运行环境并返回合适的 BackendAdapter。
 * - Tauri 桌面端 → TauriAdapter（通过 IPC 调用 Rust 命令）
 * - 浏览器/开发模式 → HttpAdapter（通过 REST 调用 http-web）
 */
export function detectAdapter(): BackendAdapter {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  if (w.__TAURI_INTERNALS__ || w.__TAURI__) {
    return new TauriAdapter();
  }
  const baseUrl = import.meta.env.VITE_BACKEND_URL ?? '';
  const token = import.meta.env.VITE_BACKEND_TOKEN ?? undefined;
  return new HttpAdapter({ baseUrl, token });
}
