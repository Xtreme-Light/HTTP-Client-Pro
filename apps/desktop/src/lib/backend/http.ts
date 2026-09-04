import type { BackendAdapter, DispatchResponse, ExecuteEvent, ExecuteOptions } from '../../types/http';

export interface HttpAdapterOptions {
  baseUrl?: string;
  token?: string;
}

/** REST adapter — talks to http-web (axum) via fetch. */
export class HttpAdapter implements BackendAdapter {
  private baseUrl: string;
  private token?: string;

  constructor(opts: HttpAdapterOptions = {}) {
    this.baseUrl = opts.baseUrl ?? '';
    this.token = opts.token;
  }

  private authHeaders(): Record<string, string> {
    return this.token ? { Authorization: `Bearer ${this.token}` } : {};
  }

  async health(): Promise<boolean> {
    try {
      const res = await fetch(`${this.baseUrl}/healthz`);
      if (!res.ok) return false;
      const json = await res.json();
      return json?.status === 'ok';
    } catch {
      return false;
    }
  }

  async execute(source: string, opts?: ExecuteOptions): Promise<DispatchResponse> {
    const res = await fetch(`${this.baseUrl}/execute`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain', ...this.authHeaders() },
      body: source,
      signal: opts?.signal,
    });
    if (!res.ok) {
      const text = await res.text().catch(() => '');
      throw new Error(`HTTP ${res.status}: ${text || res.statusText}`);
    }
    const json = await res.json();
    return json as DispatchResponse;
  }

  async *executeStream(source: string, opts?: ExecuteOptions): AsyncIterable<ExecuteEvent> {
    const url = `${this.baseUrl}/sse/execute?src=${encodeURIComponent(source)}`;
    const res = await fetch(url, {
      headers: this.authHeaders(),
      signal: opts?.signal,
    });
    if (!res.ok) {
      throw new Error(`HTTP ${res.status}: ${res.statusText}`);
    }
    if (!res.body) throw new Error('no response body');

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let eventName = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';

      for (const line of lines) {
        if (line.startsWith('event:')) {
          eventName = line.slice(6).trim();
        } else if (line.startsWith('data:')) {
          const data = line.slice(5).trim();
          if (eventName === 'done') {
            yield { type: 'done', response: JSON.parse(data) as DispatchResponse };
          } else if (eventName === 'error') {
            yield { type: 'error', message: data };
          }
        }
      }
    }
  }
}
