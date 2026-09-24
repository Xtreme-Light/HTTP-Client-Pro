/**
 * console-format — 把一次执行结果渲染成 JetBrains HTTP Client 风格的
 * Console 文本（状态行 + 完整响应头 + body/落盘提示 + 摘要）。
 *
 * 纯函数、无副作用，便于单测。二进制响应不会把原始字节塞进 Console，
 * 而是显示 `Response file saved.` + 文件名（由后端落盘后回填）。
 */
import type { DispatchResponse } from '../types/http';

/** 渲染所需的最小输入（来自一次运行记录）。 */
export interface ConsoleInput {
  method: string;
  target: string;
  response: DispatchResponse | null;
}

const REASON_PHRASES: Record<number, string> = {
  100: 'Continue',
  101: 'Switching Protocols',
  200: 'OK',
  201: 'Created',
  202: 'Accepted',
  204: 'No Content',
  206: 'Partial Content',
  301: 'Moved Permanently',
  302: 'Found',
  303: 'See Other',
  304: 'Not Modified',
  307: 'Temporary Redirect',
  308: 'Permanent Redirect',
  400: 'Bad Request',
  401: 'Unauthorized',
  403: 'Forbidden',
  404: 'Not Found',
  405: 'Method Not Allowed',
  408: 'Request Timeout',
  409: 'Conflict',
  410: 'Gone',
  413: 'Payload Too Large',
  415: 'Unsupported Media Type',
  422: 'Unprocessable Entity',
  429: 'Too Many Requests',
  500: 'Internal Server Error',
  501: 'Not Implemented',
  502: 'Bad Gateway',
  503: 'Service Unavailable',
  504: 'Gateway Timeout',
};

/** HTTP 状态码对应的原因短语，未知时回退空串。 */
export function reasonPhrase(status: number): string {
  return REASON_PHRASES[status] ?? '';
}

/** 字节数人性化：`3997` → `4 kB`（1000 进制，与 JetBrains 一致）。 */
export function humanizeSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  if (bytes < 1000) return `${bytes} B`;
  const units = ['kB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = -1;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${Math.round(value)} ${units[unit]}`;
}

/** 摘要里的内容长度：`3997 bytes (4 kB)`。 */
export function formatContentLength(bytes: number): string {
  return `${bytes} bytes (${humanizeSize(bytes)})`;
}

/** 摘要里的耗时：`3971ms (3 s 971 ms)`。 */
export function formatElapsed(ms: number): string {
  const safe = Number.isFinite(ms) && ms >= 0 ? ms : 0;
  const secs = Math.floor(safe / 1000);
  const rem = Math.round(safe % 1000);
  const human = secs > 0 ? `${secs} s ${rem} ms` : `${rem} ms`;
  return `${Math.round(safe)}ms (${human})`;
}

/** 从 headers 里大小写不敏感地取值。 */
function header(headers: Record<string, string>, name: string): string {
  const lower = name.toLowerCase();
  for (const key of Object.keys(headers)) {
    if (key.toLowerCase() === lower) return headers[key];
  }
  return '';
}

/** JSON body 美化；解析失败则原样返回。 */
function prettyBody(body: string, contentType: string): string {
  if (contentType.toLowerCase().includes('json')) {
    try {
      return JSON.stringify(JSON.parse(body), null, 2);
    } catch {
      return body;
    }
  }
  return body;
}

/**
 * 渲染 Console 文本。结构：
 *
 * ```
 * METHOD target
 *
 * HTTP/1.1 200 OK
 * header: value
 * …
 *
 * <body 或 "Response file saved.\n> name">
 *
 * Response code: 200 (OK); Time: …; Content length: …
 * ```
 */
export function formatConsole(input: ConsoleInput): string {
  const lines: string[] = [`${input.method} ${input.target}`, ''];
  const res = input.response;
  if (!res) return lines.join('\n');

  const version = res.http_version || 'HTTP/1.1';
  const reason = reasonPhrase(res.status);
  lines.push(`${version} ${res.status}${reason ? ` ${reason}` : ''}`);

  const headers = res.headers ?? {};
  for (const [name, value] of Object.entries(headers)) {
    lines.push(`${name}: ${value}`);
  }
  lines.push('');

  const contentType = header(headers, 'content-type');
  if (res.binary) {
    if (res.saved_path == null && res.save_error) {
      lines.push('Failed to save response file.');
      lines.push(`> ${res.save_error}`);
    } else {
      lines.push('Response file saved.');
      lines.push(`> ${res.file_name ?? res.saved_path ?? ''}`);
    }
  } else {
    const body = prettyBody(res.body ?? '', contentType);
    if (body.length > 0) lines.push(body);
  }
  lines.push('');

  const contentLength = res.content_length ?? byteLength(res.body ?? '');
  lines.push(
    `Response code: ${res.status}${reason ? ` (${reason})` : ''}; ` +
      `Time: ${formatElapsed(res.elapsed_ms)}; ` +
      `Content length: ${formatContentLength(contentLength)}`,
  );

  return lines.join('\n');
}

/** UTF-8 字节长度（浏览器/Node 通用）。 */
function byteLength(text: string): number {
  let n = 0;
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    if (code < 0x80) n += 1;
    else if (code < 0x800) n += 2;
    else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
      n += 4; // surrogate pair
      i++;
    } else n += 3;
  }
  return n;
}
