import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useHistoryStore, serialize, deserialize, type HistoryItem } from './history';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
});

describe('history serialize/deserialize', () => {
  it('round-trips items', () => {
    const items: HistoryItem[] = [
      { id: 1, ts: 1000, method: 'GET', target: '/a', status: 200, elapsedMs: 5, source: 'GET /a', response: null },
    ];
    const raw = serialize(items);
    const back = deserialize(raw);
    expect(back).toEqual(items);
  });

  it('returns empty for null', () => {
    expect(deserialize(null)).toEqual([]);
  });

  it('returns empty for malformed JSON', () => {
    expect(deserialize('{not json')).toEqual([]);
  });

  it('returns empty for non-array', () => {
    expect(deserialize('{"a":1}')).toEqual([]);
  });

  it('filters items without id', () => {
    const raw = JSON.stringify([
      { id: 1, method: 'GET' },
      { method: 'POST' },
    ]);
    const back = deserialize(raw);
    expect(back).toHaveLength(1);
    expect(back[0].id).toBe(1);
  });
});

describe('history store', () => {
  it('add() prepends item', () => {
    const s = useHistoryStore();
    s.add({ method: 'GET', target: '/a', status: 200, elapsedMs: 5, source: 'GET /a', response: null });
    expect(s.items).toHaveLength(1);
    expect(s.items[0].method).toBe('GET');
  });

  it('add() assigns sequential ids', () => {
    const s = useHistoryStore();
    s.add({ method: 'GET', target: '/a', status: 200, elapsedMs: 5, source: 's1', response: null });
    s.add({ method: 'POST', target: '/b', status: 201, elapsedMs: 10, source: 's2', response: null });
    // unshift: items[0] is the most recent (s2), items[1] is older (s1)
    expect(s.items[0].id).toBeGreaterThan(s.items[1].id);
  });

  it('caps at 50 items', () => {
    const s = useHistoryStore();
    for (let i = 0; i < 60; i++) {
      s.add({ method: 'GET', target: `/${i}`, status: 200, elapsedMs: 1, source: `s${i}`, response: null });
    }
    expect(s.items).toHaveLength(50);
  });

  it('clear() empties items', () => {
    const s = useHistoryStore();
    s.add({ method: 'GET', target: '/a', status: 200, elapsedMs: 1, source: 's', response: null });
    s.clear();
    expect(s.items).toHaveLength(0);
  });

  it('load() restores from localStorage', () => {
    const items: HistoryItem[] = [
      { id: 5, ts: 1000, method: 'GET', target: '/a', status: 200, elapsedMs: 5, source: 's', response: null },
    ];
    localStorage.setItem('http-client-pro:history', serialize(items));
    const s = useHistoryStore();
    s.load();
    expect(s.items).toHaveLength(1);
    expect(s.items[0].id).toBe(5);
  });
});
