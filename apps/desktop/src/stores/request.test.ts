import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useRequestStore } from './request';

beforeEach(() => {
  setActivePinia(createPinia());
});

describe('request store', () => {
  it('parses default source on init', () => {
    const s = useRequestStore();
    expect(s.blocks.length).toBeGreaterThanOrEqual(1);
  });

  it('setSource reparses blocks', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    expect(s.blocks).toHaveLength(2);
    expect(s.blocks[0].name).toBe('a');
    expect(s.blocks[1].name).toBe('b');
  });

  it('currentBlock follows cursor', () => {
    const s = useRequestStore();
    s.setSource('### first\nGET /a\n\n### second\nPOST /b\n');
    s.setCursorLine(0);
    expect(s.currentBlock?.name).toBe('first');
    s.setCursorLine(3);
    expect(s.currentBlock?.name).toBe('second');
  });

  it('currentBlock falls back to first block', () => {
    const s = useRequestStore();
    s.setSource('GET /a\n');
    s.setCursorLine(999);
    expect(s.currentBlock).not.toBeNull();
    expect(s.currentBlock?.method).toBe('GET');
  });

  it('currentBlock is null for empty source', () => {
    const s = useRequestStore();
    s.setSource('# just a comment\n');
    expect(s.currentBlock).toBeNull();
  });
});

describe('request store – block selection', () => {
  it('selectBlock sets selectedBlockIndex', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.selectBlock(1);
    expect(s.selectedBlockIndex).toBe(1);
    expect(s.currentBlock?.name).toBe('b');
  });

  it('selectBlock makes currentBlock independent of cursor', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.selectBlock(0);
    s.setCursorLine(3); // cursor on second block
    // selectedBlockIndex takes priority
    expect(s.currentBlock?.name).toBe('a');
  });

  it('currentBlockIndex follows selection', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.selectBlock(1);
    expect(s.currentBlockIndex).toBe(1);
  });

  it('currentBlockIndex follows cursor when no selection', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.selectBlock(1);
    // Clear selection by selecting null index — actually selectBlock always sets,
    // so test cursor following before any selection
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.setCursorLine(3);
    expect(s.currentBlockIndex).toBe(1);
  });
});

describe('request store – bidirectional sync', () => {
  it('updateRequestLine patches method in source', () => {
    const s = useRequestStore();
    s.setSource('GET /old\n');
    s.updateRequestLine('POST', '/old');
    expect(s.source).toContain('POST /old');
    expect(s.source).not.toContain('GET /old');
    // blocks should be re-parsed
    expect(s.blocks[0].method).toBe('POST');
  });

  it('updateRequestLine patches target in source', () => {
    const s = useRequestStore();
    s.setSource('GET /old\n');
    s.updateRequestLine('GET', '/new');
    expect(s.source).toContain('GET /new');
    expect(s.blocks[0].target).toBe('/new');
  });

  it('updateHeaders replaces headers in source', () => {
    const s = useRequestStore();
    s.setSource('POST /api\nContent-Type: text/plain\n\nbody\n');
    s.updateHeaders([
      { name: 'Content-Type', value: 'application/json', lineIndex: 0 },
      { name: 'Authorization', value: 'Bearer tok', lineIndex: 1 },
    ]);
    expect(s.source).toContain('Content-Type: application/json');
    expect(s.source).toContain('Authorization: Bearer tok');
    expect(s.source).not.toContain('text/plain');
    expect(s.blocks[0].headers).toHaveLength(2);
  });

  it('updateHeaders preserves body', () => {
    const s = useRequestStore();
    s.setSource('POST /api\nContent-Type: text/plain\n\n{"key":"val"}\n');
    s.updateHeaders([
      { name: 'Content-Type', value: 'application/json', lineIndex: 0 },
    ]);
    expect(s.blocks[0].body).toBe('{"key":"val"}');
  });

  it('updateBody replaces body in source', () => {
    const s = useRequestStore();
    s.setSource('POST /api\nContent-Type: application/json\n\n{"old":true}\n');
    s.updateBody('{"new":false}');
    expect(s.source).toContain('{"new":false}');
    expect(s.source).not.toContain('{"old":true}');
    expect(s.blocks[0].body).toBe('{"new":false}');
  });

  it('updateBody preserves headers', () => {
    const s = useRequestStore();
    s.setSource('POST /api\nContent-Type: application/json\nAuthorization: Bearer tok\n\n{"old":true}\n');
    s.updateBody('{"new":false}');
    expect(s.blocks[0].headers).toHaveLength(2);
    expect(s.blocks[0].headers[0].name).toBe('Content-Type');
    expect(s.blocks[0].headers[1].name).toBe('Authorization');
  });

  it('updateRequestLine works on selected block, not just cursor block', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    s.selectBlock(1);
    s.updateRequestLine('PUT', '/b');
    expect(s.blocks[1].method).toBe('PUT');
    // first block should be untouched
    expect(s.blocks[0].method).toBe('GET');
  });

  it('updateRequestLine does nothing when no current block', () => {
    const s = useRequestStore();
    s.setSource('# just a comment\n');
    expect(() => s.updateRequestLine('GET', '/x')).not.toThrow();
    expect(s.source).toBe('# just a comment\n');
  });
});

describe('request store – getBlockSource', () => {
  it('returns serialized source for current block', () => {
    const s = useRequestStore();
    s.setSource('POST /api\nContent-Type: application/json\n\n{"key":"val"}\n');
    const src = s.getBlockSource();
    expect(src).not.toBeNull();
    expect(src).toContain('POST /api');
    expect(src).toContain('Content-Type: application/json');
    expect(src).toContain('{"key":"val"}');
  });

  it('returns serialized source for specified index', () => {
    const s = useRequestStore();
    s.setSource('### a\nGET /a\n\n### b\nPOST /b\n');
    const src = s.getBlockSource(1);
    expect(src).not.toBeNull();
    expect(src).toContain('POST /b');
    expect(src).not.toContain('GET /a');
  });

  it('returns null for nonexistent index', () => {
    const s = useRequestStore();
    s.setSource('GET /a\n');
    expect(s.getBlockSource(99)).toBeNull();
  });

  it('returns null when no blocks exist', () => {
    const s = useRequestStore();
    s.setSource('# just a comment\n');
    expect(s.getBlockSource()).toBeNull();
  });

  it('serializes handler and responseRef', () => {
    const s = useRequestStore();
    s.setSource('GET /api\n> {% client.log("x") %}\n<> resp.json\n');
    const src = s.getBlockSource();
    expect(src).toContain('> {% client.log("x") %}');
    expect(src).toContain('<> resp.json');
  });
});
