/** DispatchResponse — mirrors `http_core::dispatch::DispatchResponse` / http-web's JSON. */
export interface DispatchResponse {
  status: number;
  headers: Record<string, string>;
  body: string;
  elapsed_ms: number;
  url: string;
  /** HTTP version of the status line, e.g. `HTTP/1.1`. */
  http_version?: string;
  /** Body size in bytes (before any text conversion). */
  content_length?: number;
  /** True when the response was a binary download saved to disk. */
  binary?: boolean;
  /** De-duplicated file name written to disk (binary responses only). */
  file_name?: string | null;
  /** Absolute path of the saved file (binary responses only). */
  saved_path?: string | null;
  /** Present when saving a binary response failed. */
  save_error?: string | null;
}

/** SSE event from `GET /sse/execute`. */
export type ExecuteEvent =
  | { type: 'done'; response: DispatchResponse }
  | { type: 'error'; message: string };

/** Options passed to `BackendAdapter.execute`. */
export interface ExecuteOptions {
  signal?: AbortSignal;
  /** Directory where binary (download) responses are saved. */
  saveDir?: string;
}

/** Platform-agnostic backend — implemented by `http.ts` (REST) and `tauri.ts` (IPC). */
export interface BackendAdapter {
  health(): Promise<boolean>;
  execute(source: string, opts?: ExecuteOptions): Promise<DispatchResponse>;
  executeStream(source: string, opts?: ExecuteOptions): AsyncIterable<ExecuteEvent>;
}
