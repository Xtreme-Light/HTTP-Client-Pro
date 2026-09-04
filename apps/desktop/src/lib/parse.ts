/**
 * 轻量 .http block 分割器 — 仅用于编辑器 UX（块识别、表单渲染）。
 * 完整语法分析在服务端 http-core::parser 完成，这里不做。
 */

const HTTP_METHODS = [
  'GET', 'HEAD', 'POST', 'PUT', 'DELETE', 'CONNECT', 'PATCH', 'OPTIONS', 'TRACE',
] as const;

export interface Block {
  /** 从 `### comment` 提取的请求名，无则为 null */
  name: string | null;
  /** 0-based 起始行号（含 `###` 行本身） */
  startLine: number;
  /** 0-based 结束行号（含，exclusive） */
  endLine: number;
  method: string;
  /** 请求目标（URL 或 path） */
  target: string;
  /** 解析出的 header 列表 */
  headers: HeaderLine[];
  /** 请求体文本（不含前后空行） */
  body: string;
  /** response handler 脚本（> {% ... %} 或 > file） */
  handler: string | null;
  /** response ref（<> file） */
  responseRef: string | null;
}

export interface HeaderLine {
  name: string;
  value: string;
  /** 该 header 在块内的 0-based 行号（相对于块起始行） */
  lineIndex: number;
}

export interface VarRef {
  name: string;
  from: number;
  to: number;
}

const METHOD_RE = new RegExp(
  `^\\s*(${HTTP_METHODS.join('|')})\\s+(\\S+)`,
  'i',
);

/**
 * 解析单个 block 内的 headers / body / handler / responseRef。
 */
function parseBlockBody(lines: string[], startIndex: number, endIndex: number): Pick<Block, 'headers' | 'body' | 'handler' | 'responseRef'> {
  const headers: HeaderLine[] = [];
  let bodyLines: string[] = [];
  let handler: string | null = null;
  let responseRef: string | null = null;
  let inBody = false;
  let relLine = 0;

  for (let i = startIndex; i < endIndex; i++, relLine++) {
    const line = lines[i];

    // handler 行: > {% ... %} 或 > file
    if (line.match(/^\s*>\s*\{%/)) {
      // 收集多行 handler 直到 %}
      let h = line;
      if (!h.includes('%}')) {
        for (let j = i + 1; j < endIndex && !h.includes('%}'); j++) {
          h += '\n' + lines[j];
          i++;
          relLine++;
        }
      }
      handler = h.replace(/^\s*>\s*/, '').trim();
      inBody = false;
      continue;
    }
    if (line.match(/^\s*>\s*\S+/)) {
      handler = line.replace(/^\s*>\s*/, '').trim();
      inBody = false;
      continue;
    }
    // response ref 行: <> file
    if (line.match(/^\s*<>\s*\S+/)) {
      responseRef = line.replace(/^\s*<>\s*/, '').trim();
      inBody = false;
      continue;
    }

    // 空行 — headers 到 body 的分隔
    if (line.trim() === '' && headers.length > 0 && !inBody) {
      inBody = true;
      continue;
    }

    if (inBody) {
      bodyLines.push(line);
    } else {
      // header 行: name: value
      const hMatch = line.match(/^([^:]+):\s*(.*)$/);
      if (hMatch && !line.match(/^\s*(#{2,}|\/\/)/)) {
        headers.push({
          name: hMatch[1].trim(),
          value: hMatch[2].trim(),
          lineIndex: relLine,
        });
      }
    }
  }

  // 去除 body 前后空行
  const body = bodyLines.join('\n').trim();

  return { headers, body, handler, responseRef };
}

/**
 * 将 .http 源码按 `###` 分隔符拆分为请求块。
 */
export function splitRequests(source: string): Block[] {
  const lines = source.split('\n');
  const blocks: Block[] = [];

  let i = 0;
  while (i < lines.length) {
    // Skip blank lines and non-separator comments at the top
    if (lines[i].trim() === '' || (lines[i].startsWith('#') && !lines[i].startsWith('###'))) {
      i++;
      continue;
    }

    // `###` separator — consume it and capture the name
    let name: string | null = null;
    let startLine = i;
    let reqLineIndex = i;
    if (lines[i].startsWith('###')) {
      const comment = lines[i].slice(3).trim();
      name = comment || null;
      i++;
      // Skip blank lines between separator and request line
      while (i < lines.length && lines[i].trim() === '') i++;
      reqLineIndex = i;
    }

    // Now we expect a request line (method + target)
    const line = lines[i] ?? '';
    const match = METHOD_RE.exec(line);
    let method: string;
    let target: string;
    if (match) {
      method = match[1].toUpperCase();
      target = match[2];
    } else if (/^\s*https?:\/\//i.test(line)) {
      // URL without method — default to GET
      method = 'GET';
      target = line.trim();
    } else {
      // Not a request line — skip to next ###
      if (i < lines.length) i++;
      continue;
    }

    // Find end of this block: next ### or end of file
    let endLine = lines.length;
    for (let j = i + 1; j < lines.length; j++) {
      if (lines[j].startsWith('###')) {
        endLine = j;
        break;
      }
    }

    const blockBody = parseBlockBody(lines, reqLineIndex + 1, endLine);

    blocks.push({
      name,
      startLine: name ? startLine : reqLineIndex,
      endLine,
      method,
      target,
      headers: blockBody.headers,
      body: blockBody.body,
      handler: blockBody.handler,
      responseRef: blockBody.responseRef,
    });
    i = endLine;
  }

  return blocks;
}

/**
 * 将 block 的表单数据序列化回 .http 文本。
 */
export function serializeBlock(block: Pick<Block, 'method' | 'target' | 'headers' | 'body' | 'handler' | 'responseRef'>): string {
  const lines: string[] = [];

  // 请求行
  lines.push(`${block.method} ${block.target}`);

  // headers
  for (const h of block.headers) {
    lines.push(`${h.name}: ${h.value}`);
  }

  // body
  if (block.body) {
    lines.push('');
    lines.push(block.body);
  }

  // handler
  if (block.handler) {
    if (block.handler.startsWith('{%')) {
      lines.push(`> ${block.handler}`);
    } else {
      lines.push(`> ${block.handler}`);
    }
  }

  // response ref
  if (block.responseRef) {
    lines.push(`<> ${block.responseRef}`);
  }

  return lines.join('\n');
}

/**
 * 将编辑后的 block 数据写回源码的指定位置。
 */
export function applyBlockEdit(source: string, block: Block, edits: Partial<Pick<Block, 'method' | 'target' | 'headers' | 'body'>>): string {
  const updatedBlock: Block = { ...block, ...edits };
  const serialized = serializeBlock(updatedBlock);

  const lines = source.split('\n');
  // block.startLine points to the ### separator (for named blocks) or the request line (for unnamed)
  // The actual request line (METHOD /target) is after the separator for named blocks
  const reqLine = block.name
    ? block.startLine + 1 // skip ### line
    : block.startLine;
  const endLine = block.endLine;

  // Preserve everything before the request line (including ### separator)
  const before = lines.slice(0, reqLine);
  // Preserve everything after the block
  const after = lines.slice(endLine);

  // Also skip blank line between separator and request line for named blocks
  // (splitRequests already handles this, so before[] may end with '' after ###)
  // We keep it as-is — the serializer doesn't add blank lines after ###

  const newLines = [...before, serialized, ...after];
  return newLines.join('\n');
}

/**
 * 在文本中查找所有 `{{var}}` 引用。
 */
export function findVariables(text: string): VarRef[] {
  const refs: VarRef[] = [];
  const re = /\{\{\s*([\w.-]+)\s*\}\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    refs.push({
      name: m[1],
      from: m.index,
      to: m.index + m[0].length,
    });
  }
  return refs;
}
