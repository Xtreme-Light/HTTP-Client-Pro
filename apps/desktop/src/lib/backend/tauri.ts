import { invoke } from '@tauri-apps/api/core';
import type { BackendAdapter, DispatchResponse, ExecuteEvent, ExecuteOptions } from '../../types/http';

export interface TauriAdapterOptions {
  /** 可注入 mock invoke 用于测试 */
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
}

/**
 * Tauri IPC adapter — 通过 Tauri invoke 直接调用 Rust http-core。
 * 在非 Tauri 环境下（如浏览器开发模式）invoke 会抛错。
 */
export class TauriAdapter implements BackendAdapter {
  private invokeFn: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

  constructor(opts: TauriAdapterOptions = {}) {
    this.invokeFn = opts.invoke ?? invoke;
  }

  async health(): Promise<boolean> {
    try {
      const res = await this.invokeFn('ping');
      return res === 'ok';
    } catch {
      return false;
    }
  }

  async execute(source: string, _opts?: ExecuteOptions): Promise<DispatchResponse> {
    const res = await this.invokeFn('execute_http', { source });
    return res as DispatchResponse;
  }

  async *executeStream(source: string, _opts?: ExecuteOptions): AsyncIterable<ExecuteEvent> {
    const res = await this.invokeFn('execute_http', { source });
    yield { type: 'done', response: res as DispatchResponse };
  }
}
