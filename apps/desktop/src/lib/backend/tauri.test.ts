import { describe, it, expect, vi } from 'vitest';
import { TauriAdapter } from './tauri';
import type { DispatchResponse } from '../../types/http';

describe('TauriAdapter', () => {
  it('execute() calls invoke("execute_http", { source })', async () => {
    const invoke = vi.fn();
    const sample: DispatchResponse = {
      status: 200, headers: {}, body: 'ok', elapsed_ms: 5, url: 'u',
    };
    invoke.mockResolvedValue(sample);
    const a = new TauriAdapter({ invoke });
    const res = await a.execute('GET /a\n');
    expect(res).toEqual(sample);
    expect(invoke).toHaveBeenCalledWith('execute_http', { source: 'GET /a\n' });
  });

  it('health() returns true when invoke succeeds', async () => {
    const invoke = vi.fn().mockResolvedValue('ok');
    const a = new TauriAdapter({ invoke });
    expect(await a.health()).toBe(true);
  });

  it('health() returns false when invoke throws', async () => {
    const invoke = vi.fn().mockRejectedValue(new Error('no tauri'));
    const a = new TauriAdapter({ invoke });
    expect(await a.health()).toBe(false);
  });

  it('executeStream() yields a done event', async () => {
    const invoke = vi.fn();
    const sample: DispatchResponse = {
      status: 201, headers: {}, body: 'created', elapsed_ms: 10, url: 'u',
    };
    invoke.mockResolvedValue(sample);
    const a = new TauriAdapter({ invoke });
    const events = [];
    for await (const e of a.executeStream('POST /a\n')) {
      events.push(e);
    }
    expect(events).toHaveLength(1);
    expect(events[0].type).toBe('done');
    if (events[0].type === 'done') {
      expect(events[0].response.status).toBe(201);
    }
  });
});
