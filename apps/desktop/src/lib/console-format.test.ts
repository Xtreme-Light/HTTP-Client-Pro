import { describe, it, expect } from 'vitest';
import {
  reasonPhrase,
  humanizeSize,
  formatContentLength,
  formatElapsed,
  formatConsole,
} from './console-format';
import type { DispatchResponse } from '../types/http';

describe('reasonPhrase', () => {
  it('maps known statuses', () => {
    expect(reasonPhrase(200)).toBe('OK');
    expect(reasonPhrase(404)).toBe('Not Found');
    expect(reasonPhrase(500)).toBe('Internal Server Error');
  });
  it('falls back to empty for unknown', () => {
    expect(reasonPhrase(599)).toBe('');
  });
});

describe('humanizeSize', () => {
  it('uses bytes below 1000', () => {
    expect(humanizeSize(0)).toBe('0 B');
    expect(humanizeSize(999)).toBe('999 B');
  });
  it('rounds to kB/MB (1000-base)', () => {
    expect(humanizeSize(3997)).toBe('4 kB');
    expect(humanizeSize(1_500_000)).toBe('2 MB');
  });
});

describe('formatContentLength', () => {
  it('shows raw bytes plus humanized', () => {
    expect(formatContentLength(3997)).toBe('3997 bytes (4 kB)');
  });
});

describe('formatElapsed', () => {
  it('splits seconds and millis', () => {
    expect(formatElapsed(3971)).toBe('3971ms (3 s 971 ms)');
    expect(formatElapsed(120)).toBe('120ms (120 ms)');
  });
});

function resp(overrides: Partial<DispatchResponse> = {}): DispatchResponse {
  return {
    status: 200,
    headers: {},
    body: '',
    elapsed_ms: 0,
    url: 'https://x.test/api',
    http_version: 'HTTP/1.1',
    ...overrides,
  };
}

describe('formatConsole', () => {
  it('renders the request line and status line with headers', () => {
    const out = formatConsole({
      method: 'GET',
      target: 'https://x.test/api',
      response: resp({
        status: 200,
        http_version: 'HTTP/1.1',
        headers: { 'Content-Type': 'text/plain', Vary: 'Origin' },
        body: 'hello',
        elapsed_ms: 5,
        content_length: 5,
      }),
    });
    const lines = out.split('\n');
    expect(lines[0]).toBe('GET https://x.test/api');
    expect(lines[2]).toBe('HTTP/1.1 200 OK');
    expect(out).toContain('Content-Type: text/plain');
    expect(out).toContain('Vary: Origin');
    expect(out).toContain('hello');
    expect(lines[lines.length - 1]).toBe(
      'Response code: 200 (OK); Time: 5ms (5 ms); Content length: 5 bytes (5 B)',
    );
  });

  it('saves binary responses to a file instead of dumping bytes', () => {
    const out = formatConsole({
      method: 'POST',
      target: 'https://x.test/import',
      response: resp({
        status: 200,
        binary: true,
        file_name: '错误信息-5.xlsx',
        saved_path: '/ws/.http-history/错误信息-5.xlsx',
        body: '',
        content_length: 3997,
        elapsed_ms: 3971,
        headers: {
          'Content-disposition': "attachment;filename*=utf-8''%E9%94%99.xlsx",
          'Content-Type':
            'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;charset=utf-8',
        },
      }),
    });
    expect(out).not.toContain('PK');
    expect(out).toContain('Response file saved.');
    expect(out).toContain('> 错误信息-5.xlsx');
    expect(out).toContain(
      'Response code: 200 (OK); Time: 3971ms (3 s 971 ms); Content length: 3997 bytes (4 kB)',
    );
  });

  it('reports a save failure', () => {
    const out = formatConsole({
      method: 'GET',
      target: 'https://x.test/file',
      response: resp({
        status: 200,
        binary: true,
        saved_path: null,
        file_name: 'a.bin',
        save_error: 'permission denied',
      }),
    });
    expect(out).toContain('Failed to save response file.');
    expect(out).toContain('> permission denied');
  });

  it('pretty-prints JSON bodies', () => {
    const out = formatConsole({
      method: 'GET',
      target: 'https://x.test/json',
      response: resp({
        status: 200,
        headers: { 'Content-Type': 'application/json' },
        body: '{"a":1}',
        content_length: 7,
      }),
    });
    expect(out).toContain('{\n  "a": 1\n}');
  });

  it('handles a missing response', () => {
    const out = formatConsole({ method: 'GET', target: 'https://x.test', response: null });
    expect(out).toBe('GET https://x.test\n');
  });

  it('computes content length from the body when absent', () => {
    const out = formatConsole({
      method: 'GET',
      target: 'https://x.test',
      response: resp({ status: 204, body: '', headers: {} }),
    });
    expect(out).toContain('Content length: 0 bytes (0 B)');
  });
});
