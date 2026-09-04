import { describe, it, expect, vi, beforeEach } from 'vitest';
import { HttpAdapter } from './http';
import type { DispatchResponse } from '../../types/http';

function mockResponse(data: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    statusText: ok ? 'OK' : 'Error',
    headers: new Headers(),
    json: () => Promise.resolve(data),
    text: () => Promise.resolve(typeof data === 'string' ? data : JSON.stringify(data)),
    body: null,
  } as Response;
}

const sampleResponse: DispatchResponse = {
  status: 200,
  headers: { 'content-type': 'text/plain' },
  body: 'hello',
  elapsed_ms: 42,
  url: 'http://example.com/api',
};

describe('HttpAdapter', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('health() returns true when status=ok', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(mockResponse({ status: 'ok' })));
    const a = new HttpAdapter();
    expect(await a.health()).toBe(true);
  });

  it('health() returns false on network error', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('network')));
    const a = new HttpAdapter();
    expect(await a.health()).toBe(false);
  });

  it('execute() sends POST with text/plain body', async () => {
    const fetchMock = vi.fn().mockResolvedValue(mockResponse(sampleResponse));
    vi.stubGlobal('fetch', fetchMock);
    const a = new HttpAdapter({ baseUrl: 'http://localhost:8080' });
    const res = await a.execute('GET /api\n');
    expect(res).toEqual(sampleResponse);
    expect(fetchMock).toHaveBeenCalledWith(
      'http://localhost:8080/execute',
      expect.objectContaining({
        method: 'POST',
        body: 'GET /api\n',
        headers: expect.objectContaining({ 'Content-Type': 'text/plain' }),
      }),
    );
  });

  it('execute() sends Bearer token when configured', async () => {
    const fetchMock = vi.fn().mockResolvedValue(mockResponse(sampleResponse));
    vi.stubGlobal('fetch', fetchMock);
    const a = new HttpAdapter({ token: 's3cret' });
    await a.execute('GET /api\n');
    const call = fetchMock.mock.calls[0];
    expect(call[1].headers.Authorization).toBe('Bearer s3cret');
  });

  it('execute() throws on non-2xx', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(mockResponse('bad request', false, 400)));
    const a = new HttpAdapter();
    await expect(a.execute('GET /api\n')).rejects.toThrow('HTTP 400');
  });

  it('detectAdapter() returns HttpAdapter in browser (no __TAURI__)', async () => {
    vi.stubGlobal('window', { __TAURI__: undefined });
    const { detectAdapter } = await import('./index');
    const adapter = detectAdapter();
    expect(adapter.constructor.name).toBe('HttpAdapter');
  });

  it('detectAdapter() returns TauriAdapter when __TAURI__ is present', async () => {
    vi.stubGlobal('window', { __TAURI__: {} });
    const { detectAdapter } = await import('./index');
    const adapter = detectAdapter();
    expect(adapter.constructor.name).toBe('TauriAdapter');
  });
});
