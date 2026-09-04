import { describe, it, expect } from 'vitest';
import { splitRequests, findVariables, serializeBlock, applyBlockEdit, type Block, type VarRef } from './parse';

describe('splitRequests', () => {
  it('splits a single unnamed request', () => {
    const src = 'GET http://example.com/api\n';
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].name).toBeNull();
    expect(blocks[0].method).toBe('GET');
    expect(blocks[0].target).toBe('http://example.com/api');
  });

  it('splits multiple requests separated by ###', () => {
    const src = [
      '### login',
      'POST http://example.com/auth',
      '',
      '### get-user',
      'GET http://example.com/me',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(2);
    expect(blocks[0].name).toBe('login');
    expect(blocks[0].method).toBe('POST');
    expect(blocks[1].name).toBe('get-user');
    expect(blocks[1].method).toBe('GET');
  });

  it('handles unnamed request after separator', () => {
    const src = '###\nGET http://example.com/a\n';
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].name).toBeNull();
  });

  it('returns empty for pure comments', () => {
    const src = '# just a comment\n// another\n';
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(0);
  });

  it('returns empty for empty input', () => {
    expect(splitRequests('')).toEqual([]);
    expect(splitRequests('\n\n')).toEqual([]);
  });

  it('computes correct line ranges', () => {
    const src = 'GET /a\n\n### second\nPOST /b\n';
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(2);
    expect(blocks[0].startLine).toBe(0);
    expect(blocks[1].startLine).toBe(2); // ### second is line 2 (0-based)
  });

  it('detects default GET when no method prefix', () => {
    const src = 'http://example.com/api\n';
    const blocks = splitRequests(src);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].method).toBe('GET');
    expect(blocks[0].target).toBe('http://example.com/api');
  });

  it('extracts all 9 HTTP methods', () => {
    const methods = ['GET', 'HEAD', 'POST', 'PUT', 'DELETE', 'CONNECT', 'PATCH', 'OPTIONS', 'TRACE'];
    for (const m of methods) {
      const blocks = splitRequests(`${m} http://example.com/\n`);
      expect(blocks[0].method, `${m} should parse`).toBe(m);
    }
  });

  it('parses headers from a block', () => {
    const src = [
      'POST /api',
      'Content-Type: application/json',
      'Authorization: Bearer token123',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].headers).toHaveLength(2);
    expect(blocks[0].headers[0]).toMatchObject({ name: 'Content-Type', value: 'application/json' });
    expect(blocks[0].headers[1]).toMatchObject({ name: 'Authorization', value: 'Bearer token123' });
  });

  it('parses body after headers', () => {
    const src = [
      'POST /api',
      'Content-Type: application/json',
      '',
      '{ "key": "value" }',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].body).toBe('{ "key": "value" }');
  });

  it('parses multi-line body', () => {
    const src = [
      'POST /api',
      'Content-Type: application/json',
      '',
      '{',
      '  "key": "value",',
      '  "num": 42',
      '}',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].body).toContain('"key": "value"');
    expect(blocks[0].body).toContain('"num": 42');
  });

  it('parses handler line with > {% ... %}', () => {
    const src = [
      'GET /api',
      '> {% client.log("test") %}',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].handler).toBe('{% client.log("test") %}');
  });

  it('parses handler line with > file reference', () => {
    const src = [
      'GET /api',
      '> handler.js',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].handler).toBe('handler.js');
  });

  it('parses response ref line with <>', () => {
    const src = [
      'GET /api',
      '<> response.json',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].responseRef).toBe('response.json');
  });

  it('parses full block with headers, body, handler, and responseRef', () => {
    const src = [
      '### full request',
      'POST /api/v1',
      'Content-Type: application/json',
      'Authorization: Bearer {{token}}',
      '',
      '{ "user": "admin" }',
      '> {% client.log("done") %}',
      '<> expected.json',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].name).toBe('full request');
    expect(blocks[0].method).toBe('POST');
    expect(blocks[0].target).toBe('/api/v1');
    expect(blocks[0].headers).toHaveLength(2);
    expect(blocks[0].body).toBe('{ "user": "admin" }');
    expect(blocks[0].handler).toBe('{% client.log("done") %}');
    expect(blocks[0].responseRef).toBe('expected.json');
  });

  it('does not treat comment lines as headers', () => {
    const src = [
      'GET /api',
      '# This is a comment',
      '// Another comment',
      'Accept: application/json',
    ].join('\n');
    const blocks = splitRequests(src);
    expect(blocks[0].headers).toHaveLength(1);
    expect(blocks[0].headers[0].name).toBe('Accept');
  });
});

describe('serializeBlock', () => {
  it('serializes a simple GET request', () => {
    const result = serializeBlock({
      method: 'GET',
      target: 'http://example.com/api',
      headers: [],
      body: '',
      handler: null,
      responseRef: null,
    });
    expect(result).toBe('GET http://example.com/api');
  });

  it('serializes request with headers', () => {
    const result = serializeBlock({
      method: 'POST',
      target: '/api',
      headers: [
        { name: 'Content-Type', value: 'application/json', lineIndex: 0 },
        { name: 'Authorization', value: 'Bearer tok', lineIndex: 1 },
      ],
      body: '',
      handler: null,
      responseRef: null,
    });
    expect(result).toBe([
      'POST /api',
      'Content-Type: application/json',
      'Authorization: Bearer tok',
    ].join('\n'));
  });

  it('serializes request with body', () => {
    const result = serializeBlock({
      method: 'POST',
      target: '/api',
      headers: [{ name: 'Content-Type', value: 'application/json', lineIndex: 0 }],
      body: '{"key":"value"}',
      handler: null,
      responseRef: null,
    });
    expect(result).toBe([
      'POST /api',
      'Content-Type: application/json',
      '',
      '{"key":"value"}',
    ].join('\n'));
  });

  it('serializes request with handler', () => {
    const result = serializeBlock({
      method: 'GET',
      target: '/api',
      headers: [],
      body: '',
      handler: '{% client.log("hi") %}',
      responseRef: null,
    });
    expect(result).toBe('GET /api\n> {% client.log("hi") %}');
  });

  it('serializes request with responseRef', () => {
    const result = serializeBlock({
      method: 'GET',
      target: '/api',
      headers: [],
      body: '',
      handler: null,
      responseRef: 'expected.json',
    });
    expect(result).toBe('GET /api\n<> expected.json');
  });

  it('serializes full block with all fields', () => {
    const result = serializeBlock({
      method: 'POST',
      target: '/api',
      headers: [{ name: 'Accept', value: 'application/json', lineIndex: 0 }],
      body: '{"data":1}',
      handler: 'handler.js',
      responseRef: 'resp.json',
    });
    expect(result).toBe([
      'POST /api',
      'Accept: application/json',
      '',
      '{"data":1}',
      '> handler.js',
      '<> resp.json',
    ].join('\n'));
  });

  it('round-trips through splitRequests and serializeBlock', () => {
    const src = [
      'POST /api',
      'Content-Type: application/json',
      '',
      '{"key":"value"}',
      '> {% client.log("x") %}',
    ].join('\n');
    const blocks = splitRequests(src);
    const serialized = serializeBlock(blocks[0]);
    const reBlocks = splitRequests(serialized + '\n');
    expect(reBlocks[0].method).toBe('POST');
    expect(reBlocks[0].target).toBe('/api');
    expect(reBlocks[0].headers).toHaveLength(1);
    expect(reBlocks[0].body).toBe('{"key":"value"}');
    expect(reBlocks[0].handler).toBe('{% client.log("x") %}');
  });
});

describe('applyBlockEdit', () => {
  it('patches method in unnamed single block', () => {
    const src = 'GET /api\n';
    const blocks = splitRequests(src);
    const result = applyBlockEdit(src, blocks[0], { method: 'POST' });
    expect(result).toContain('POST /api');
    expect(result).not.toContain('GET /api');
  });

  it('patches target in unnamed single block', () => {
    const src = 'GET /old\n';
    const blocks = splitRequests(src);
    const result = applyBlockEdit(src, blocks[0], { target: '/new' });
    expect(result).toContain('GET /new');
    expect(result).not.toContain('GET /old');
  });

  it('patches headers in a block', () => {
    const src = [
      'POST /api',
      'Content-Type: text/plain',
      '',
      'body',
    ].join('\n');
    const blocks = splitRequests(src);
    const result = applyBlockEdit(src, blocks[0], {
      headers: [
        { name: 'Content-Type', value: 'application/json', lineIndex: 0 },
        { name: 'Authorization', value: 'Bearer tok', lineIndex: 1 },
      ],
    });
    const reBlocks = splitRequests(result);
    expect(reBlocks[0].headers).toHaveLength(2);
    expect(reBlocks[0].headers[0].value).toBe('application/json');
    expect(reBlocks[0].headers[1].name).toBe('Authorization');
    // body should be preserved
    expect(reBlocks[0].body).toBe('body');
  });

  it('patches body in a block', () => {
    const src = [
      'POST /api',
      'Content-Type: application/json',
      '',
      '{"old":true}',
    ].join('\n');
    const blocks = splitRequests(src);
    const result = applyBlockEdit(src, blocks[0], { body: '{"new":false}' });
    const reBlocks = splitRequests(result);
    expect(reBlocks[0].body).toBe('{"new":false}');
    // headers should be preserved
    expect(reBlocks[0].headers[0].name).toBe('Content-Type');
  });

  it('preserves separator comments for named blocks', () => {
    const src = [
      '### login',
      'POST /auth',
      '',
      '### get-user',
      'GET /me',
    ].join('\n');
    const blocks = splitRequests(src);
    // Edit the second block (get-user)
    const result = applyBlockEdit(src, blocks[1], { method: 'POST' });
    expect(result).toContain('### login');
    expect(result).toContain('### get-user');
    const reBlocks = splitRequests(result);
    expect(reBlocks).toHaveLength(2);
    expect(reBlocks[1].method).toBe('POST');
    expect(reBlocks[0].method).toBe('POST');
  });

  it('does not corrupt adjacent blocks', () => {
    const src = [
      '### first',
      'GET /a',
      '',
      '### second',
      'POST /b',
    ].join('\n');
    const blocks = splitRequests(src);
    const result = applyBlockEdit(src, blocks[0], { target: '/a-updated' });
    const reBlocks = splitRequests(result);
    expect(reBlocks).toHaveLength(2);
    expect(reBlocks[0].target).toBe('/a-updated');
    expect(reBlocks[1].target).toBe('/b');
  });
});

describe('findVariables', () => {
  it('finds {{var}} references', () => {
    const refs = findVariables('http://{{host}}/api');
    expect(refs).toHaveLength(1);
    expect(refs[0].name).toBe('host');
    expect(refs[0].from).toBe(7);
  });

  it('finds multiple variables', () => {
    const refs = findVariables('{{a}}/{{b}}');
    expect(refs.map((r) => r.name)).toEqual(['a', 'b']);
  });

  it('handles whitespace inside braces', () => {
    const refs = findVariables('{{ host }}');
    expect(refs).toHaveLength(1);
    expect(refs[0].name).toBe('host');
  });

  it('returns empty for no variables', () => {
    expect(findVariables('http://example.com/api')).toEqual([]);
  });

  it('ignores unclosed braces', () => {
    expect(findVariables('{{host')).toEqual([]);
  });
});
