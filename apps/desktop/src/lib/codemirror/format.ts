/**
 * .http 文档格式化 — 扫描各请求块的 body 区域，对 JSON body 做美化（pretty-print）。
 *
 * body 区域识别规则与语言高亮（lang-http.ts）保持一致：
 * 请求行之后遇到空行即进入 body，遇到 trailer 行（`> {% %}` / `<>` / `>>` / `< file`）
 * 或下一个 `###` 分隔符即结束。
 *
 * JSON 中的 `{{var}}` 在解析前被替换为占位符（字符串内 / 字符串外使用不同前缀，
 * 避免 `{"a":"{{v}}"}` 还原时丢失引号），格式化后再原样还原。
 */

import type { EditorView } from '@codemirror/view';

/** 一个请求块的 body 区域（0-based 行号，含端点） */
export interface BodyRegion {
  startLine: number;
  endLine: number;
  /** 该块 header 中的 Content-Type 原值，未声明则为 null */
  contentType: string | null;
}

const METHOD_RE = /^\s*(GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE)\s+\S/i;
const BARE_URL_RE = /^\s*https?:\/\/\S/i;
/** trailer 行 — response handler / response ref / 输出重定向 / 文件 body */
const TRAILER_RE = /^\s*(?:>>\s*\S|<>\s*\S|>\s*\S|<\s+\S)/;
const HEADER_RE = /^\s*([^:\s][^:]*):\s*(.*)$/;
const VAR_RE = /^\{\{\s*[\w.-]+\s*\}\}/;

/**
 * 扫描文档行，返回所有 body 区域。
 * 区域不含首尾空行（body 内部的空行保留在区间内）。
 */
export function findBodyRegions(lines: string[]): BodyRegion[] {
  const regions: BodyRegion[] = [];
  let seenRequest = false;
  let inBody = false;
  let startLine = -1;
  let endLine = -1;
  let contentType: string | null = null;

  const flush = () => {
    if (inBody && startLine >= 0 && endLine >= startLine) {
      regions.push({ startLine, endLine, contentType });
    }
    inBody = false;
    startLine = -1;
    endLine = -1;
    contentType = null;
  };

  for (let i = 0; i < lines.length; i++) {
    const text = lines[i];

    // `###` 分隔符 — 结束当前区域并重置块状态
    if (text.startsWith('###')) {
      flush();
      seenRequest = false;
      continue;
    }

    if (inBody) {
      if (TRAILER_RE.test(text)) {
        flush();
        seenRequest = false;
        continue;
      }
      // 空行不扩展区域 — 自动去掉 body 尾部空行
      if (text.trim() === '') continue;
      if (startLine < 0) startLine = i;
      endLine = i;
      continue;
    }

    // 空行 — 请求行/头部结束，其后进入 body
    if (text.trim() === '') {
      if (seenRequest) inBody = true;
      continue;
    }

    // trailer 行出现在 body 之前（无 body 的块）— 本块不再有 body
    if (TRAILER_RE.test(text)) {
      seenRequest = false;
      continue;
    }

    if (!seenRequest) {
      if (METHOD_RE.test(text) || BARE_URL_RE.test(text)) seenRequest = true;
      continue;
    }

    // header 行 — 捕获 Content-Type
    const m = HEADER_RE.exec(text);
    if (m && m[1].toLowerCase() === 'content-type') {
      contentType = m[2].trim();
    }
  }

  flush();
  return regions;
}

/** body 是否为 JSON — 依据 Content-Type 或内容形状判断 */
export function isJsonBody(text: string, contentType: string | null): boolean {
  if (contentType && /json/i.test(contentType)) return true;
  const t = text.trim();
  return t.startsWith('{') || t.startsWith('[');
}

interface MaskResult {
  text: string;
  /** 字符串外的变量（还原时不带引号） */
  bare: string[];
  /** 字符串内的变量（还原时保留外层引号） */
  str: string[];
}

/** 从 start 处的 `{{` 起匹配完整变量，返回结束位置（不含），不匹配返回 -1 */
function varEndAt(src: string, start: number): number {
  const m = VAR_RE.exec(src.slice(start));
  return m ? start + m[0].length : -1;
}

/**
 * 将 JSON 文本中的 `{{var}}` 替换为合法 JSON 占位符。
 * 字符串外的变量替换为带引号的独立占位符，字符串内的替换为无引号占位符，
 * 两种前缀（HCB / HCS）互不干扰，还原时不会产生歧义。
 */
function maskVariables(src: string): MaskResult {
  const bare: string[] = [];
  const str: string[] = [];
  let out = '';
  let inString = false;

  for (let i = 0; i < src.length; ) {
    const ch = src[i];

    if (inString) {
      if (ch === '\\') {
        out += src.slice(i, i + 2);
        i += 2;
        continue;
      }
      if (ch === '"') {
        inString = false;
        out += ch;
        i++;
        continue;
      }
      if (ch === '{' && src[i + 1] === '{') {
        const end = varEndAt(src, i);
        if (end > i) {
          str.push(src.slice(i, end));
          out += `@@HCS${str.length - 1}@@`;
          i = end;
          continue;
        }
      }
      out += ch;
      i++;
      continue;
    }

    if (ch === '"') {
      inString = true;
      out += ch;
      i++;
      continue;
    }
    if (ch === '{' && src[i + 1] === '{') {
      const end = varEndAt(src, i);
      if (end > i) {
        bare.push(src.slice(i, end));
        out += `"@@HCB${bare.length - 1}@@"`;
        i = end;
        continue;
      }
    }
    out += ch;
    i++;
  }

  return { text: out, bare, str };
}

function unmaskVariables(text: string, masked: MaskResult): string {
  let out = text;
  masked.bare.forEach((v, i) => {
    out = out.split(`"@@HCB${i}@@"`).join(v);
  });
  masked.str.forEach((v, i) => {
    out = out.split(`@@HCS${i}@@`).join(v);
  });
  return out;
}

/**
 * 格式化 JSON 文本（保留 `{{var}}`）。
 * 无法解析为 JSON 时返回 null。
 */
export function formatJson(text: string, indent = 2): string | null {
  if (text.trim() === '') return null;
  const masked = maskVariables(text);
  let parsed: unknown;
  try {
    parsed = JSON.parse(masked.text);
  } catch {
    return null;
  }
  let pretty: string;
  try {
    pretty = JSON.stringify(parsed, null, indent);
  } catch {
    return null;
  }
  if (typeof pretty !== 'string') return null;
  return unmaskVariables(pretty, masked);
}

/** 一处字符级替换（偏移基于整篇文档） */
export interface FormatEdit {
  from: number;
  to: number;
  insert: string;
}

/**
 * 计算文档中所有需要格式化的 body 区域，返回按位置升序的编辑列表。
 * 非 JSON body、解析失败的 body、已经格式化好的 body 都不产生编辑。
 */
export function formatHttpEdits(source: string, indent = 2): FormatEdit[] {
  const eol = source.includes('\r\n') ? '\r\n' : '\n';
  const lines = source.split(/\r?\n/);

  const lineStarts: number[] = [];
  let offset = 0;
  for (const line of lines) {
    lineStarts.push(offset);
    offset += line.length + eol.length;
  }

  const edits: FormatEdit[] = [];
  for (const region of findBodyRegions(lines)) {
    const bodyLines = lines.slice(region.startLine, region.endLine + 1);
    const original = bodyLines.join(eol);
    if (!isJsonBody(original, region.contentType)) continue;
    const formatted = formatJson(bodyLines.join('\n'), indent);
    if (formatted === null) continue;
    const insert = eol === '\n' ? formatted : formatted.split('\n').join(eol);
    if (insert === original) continue;
    edits.push({
      from: lineStarts[region.startLine],
      to: lineStarts[region.endLine] + lines[region.endLine].length,
      insert,
    });
  }
  return edits;
}

/** 对整篇文档应用格式化，返回新文本与实际格式化的 body 数量 */
export function formatHttpSource(source: string, indent = 2): { text: string; formatted: number } {
  const edits = formatHttpEdits(source, indent);
  let text = source;
  // 倒序应用，避免前面的替换影响后面的偏移
  for (let i = edits.length - 1; i >= 0; i--) {
    const e = edits[i];
    text = text.slice(0, e.from) + e.insert + text.slice(e.to);
  }
  return { text, formatted: edits.length };
}

/**
 * CodeMirror 命令 — 格式化文档中的所有 JSON body。
 * 无可格式化内容时返回 false（不进入撤销栈，也不吞掉按键）。
 */
export function formatDocumentCommand(view: EditorView): boolean {
  const edits = formatHttpEdits(view.state.sliceDoc());
  if (edits.length === 0) return false;
  view.dispatch({
    changes: edits.map((e) => ({ from: e.from, to: e.to, insert: e.insert })),
    userEvent: 'input.format',
  });
  return true;
}
