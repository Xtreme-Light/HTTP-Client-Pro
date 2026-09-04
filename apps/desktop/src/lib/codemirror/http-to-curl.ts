/**
 * spec(.http) 请求块 → bash curl 命令转换（Copy as cURL 用）。
 *
 * 规则：
 * - 非 GET 方法输出 `-X METHOD`
 * - target / header 值 / body 中的 `{{var}}` 通过 `resolveVar` 解析；
 *   未定义的变量保留原始 `{{var}}` 形式
 * - 字符串统一使用单引号包裹并做 bash 转义
 * - response handler（> ...）与 response ref（<> ...）无 curl 对应，忽略
 */

import type { Block } from '../parse';

export interface HttpToCurlOptions {
  /** 解析 `{{var}}` 环境变量；返回 undefined 表示未定义，保留原样 */
  resolveVar?: (name: string) => string | undefined;
}

/** bash 单引号转义：' → '\'' */
function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

function substituteVars(text: string, resolveVar?: (name: string) => string | undefined): string {
  if (!resolveVar) return text;
  return text.replace(/\{\{\s*([\w.-]+)\s*\}\}/g, (m, name: string) => resolveVar(name) ?? m);
}

/** 将请求块转换为格式化的多行 curl 命令 */
export function blockToCurl(block: Pick<Block, 'method' | 'target' | 'headers' | 'body'>, opts: HttpToCurlOptions = {}): string {
  const { resolveVar } = opts;
  const lines: string[] = [];

  const head = block.method !== 'GET' ? `curl -X ${block.method}` : 'curl';
  lines.push(`${head} ${shellQuote(substituteVars(block.target, resolveVar))}`);

  for (const h of block.headers) {
    if (!h.name.trim()) continue;
    lines.push(`-H ${shellQuote(`${h.name}: ${substituteVars(h.value, resolveVar)}`)}`);
  }

  if (block.body) {
    lines.push(`--data-raw ${shellQuote(substituteVars(block.body, resolveVar))}`);
  }

  if (lines.length === 1) return lines[0];
  return lines.join(' \\\n  ');
}
