import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useResponseStore } from './response';

beforeEach(() => {
  setActivePinia(createPinia());
});

describe('response store', () => {
  it('starts in idle state', () => {
    const s = useResponseStore();
    expect(s.loading).toBe(false);
    expect(s.status).toBeNull();
    expect(s.error).toBeNull();
  });

  it('start() sets loading', () => {
    const s = useResponseStore();
    s.start();
    expect(s.loading).toBe(true);
    expect(s.error).toBeNull();
  });

  it('ok() populates response fields', () => {
    const s = useResponseStore();
    s.start();
    s.ok({
      status: 200,
      headers: { 'content-type': 'application/json' },
      body: '{"ok":true}',
      elapsed_ms: 42,
      url: 'http://example.com/api',
    });
    expect(s.loading).toBe(false);
    expect(s.status).toBe(200);
    expect(s.headers['content-type']).toBe('application/json');
    expect(s.body).toBe('{"ok":true}');
    expect(s.elapsedMs).toBe(42);
  });

  it('fail() sets error and clears loading', () => {
    const s = useResponseStore();
    s.start();
    s.fail('network error');
    expect(s.loading).toBe(false);
    expect(s.error).toBe('network error');
  });

  it('reset() clears everything', () => {
    const s = useResponseStore();
    s.ok({ status: 200, headers: {}, body: 'x', elapsed_ms: 1, url: 'u' });
    s.reset();
    expect(s.status).toBeNull();
    expect(s.body).toBe('');
  });
});
