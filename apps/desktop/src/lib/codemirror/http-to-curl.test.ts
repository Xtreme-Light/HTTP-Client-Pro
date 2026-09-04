import { describe, it, expect } from 'vitest';
import { blockToCurl } from './http-to-curl';
import { splitRequests } from '../parse';

function blockFromSource(source: string) {
  const blocks = splitRequests(source);
  expect(blocks.length).toBeGreaterThan(0);
  return blocks[0];
}

describe('blockToCurl', () => {
  it('GET 请求仅输出 URL', () => {
    const block = blockFromSource('### 登录\nGET https://example.com/api\n');
    expect(blockToCurl(block)).toBe(`curl 'https://example.com/api'`);
  });

  it('非 GET 输出 -X', () => {
    const block = blockFromSource('POST https://example.com/api\n');
    expect(blockToCurl(block)).toBe(`curl -X POST 'https://example.com/api'`);
  });

  it('包含 headers 与 body 并续行', () => {
    const block = blockFromSource([
      'POST https://example.com/login',
      'Content-Type: application/json',
      'X-Token: abc',
      '',
      '{"user": "a"}',
    ].join('\n'));
    expect(blockToCurl(block)).toBe([
      `curl -X POST 'https://example.com/login' \\`,
      `  -H 'Content-Type: application/json' \\`,
      `  -H 'X-Token: abc' \\`,
      `  --data-raw '{"user": "a"}'`,
    ].join('\n'));
  });

  it('单引号转义', () => {
    const block = blockFromSource(`POST https://example.com/q\nContent-Type: application/json\n\n{"note": "it's ok"}`);
    expect(blockToCurl(block)).toContain(`--data-raw '{"note": "it'\\''s ok"}'`);
  });

  it('替换已定义变量，未定义变量保留原样', () => {
    const block = blockFromSource('GET {{host}}/users?id={{id}}\n');
    const curl = blockToCurl(block, {
      resolveVar: (name) => (name === 'host' ? 'https://example.com' : undefined),
    });
    expect(curl).toBe(`curl 'https://example.com/users?id={{id}}'`);
  });

  it('变量替换作用于 header 与 body', () => {
    const block = blockFromSource([
      'POST {{host}}/login',
      'Authorization: Bearer {{token}}',
      '',
      '{"token": "{{token}}"}',
    ].join('\n'));
    const curl = blockToCurl(block, {
      resolveVar: (name) => (name === 'host' ? 'https://h.test' : name === 'token' ? 't0k' : undefined),
    });
    expect(curl).toContain(`'https://h.test/login'`);
    expect(curl).toContain(`-H 'Authorization: Bearer t0k'`);
    expect(curl).toContain(`--data-raw '{"token": "t0k"}'`);
  });
});
