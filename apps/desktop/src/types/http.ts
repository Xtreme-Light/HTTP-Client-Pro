/** DispatchResponse — mirrors `http_core::dispatch::DispatchResponse` / http-web's JSON. */
export interface DispatchResponse {
  status: number;
  headers: Record<string, string>;
  body: string;
  elapsed_ms: number;
  url: string;
}

/** SSE event from `GET /sse/execute`. */
export type ExecuteEvent =
  | { type: 'done'; response: DispatchResponse }
  | { type: 'error'; message: string };

/** Options passed to `BackendAdapter.execute`. */
export interface ExecuteOptions {
  signal?: AbortSignal;
}

/** Platform-agnostic backend — implemented by `http.ts` (REST) and `tauri.ts` (IPC). */
export interface BackendAdapter {
  health(): Promise<boolean>;
  execute(source: string, opts?: ExecuteOptions): Promise<DispatchResponse>;
  executeStream(source: string, opts?: ExecuteOptions): AsyncIterable<ExecuteEvent>;
}
