/**
 * .http 编辑器自动补全 —— 上下文识别 + 词典 + 候选项构造（纯函数）+ CodeMirror 适配层。
 *
 * 支持五类补全：
 * - `method`      —— 请求行行首的 HTTP 方法
 * - `url`         —— 请求目标；语料来自当前文档、其它标签页与历史记录中学习到的 URL
 * - `headerName`  —— 常见请求头 + 当前文档已用过的头名
 * - `headerValue` —— 已知头名的常见取值（如 `Content-Type: application/json`）
 * - `variable`    —— `{{...}}` 内的环境变量名与 `$` 动态变量
 */

import { autocompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';
import { splitRequests } from '../parse';

/* ------------------------------------------------------------------ */
/* 词典                                                                */
/* ------------------------------------------------------------------ */

/** HTTP 方法 —— 按使用频率排序（同时用于识别请求行） */
export const METHOD_COMPLETIONS = [
  'GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'TRACE', 'CONNECT',
];

const METHOD_ALT = 'GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE';

export interface HeaderSpec {
  name: string;
  /** 列表右侧的说明文字 */
  detail?: string;
  /** 该头的常见取值 */
  values?: string[];
}

/** 常见请求头及其典型取值 */
export const COMMON_HEADERS: HeaderSpec[] = [
  {
    name: 'Content-Type',
    detail: '请求体媒体类型',
    values: [
      'application/json',
      'application/x-www-form-urlencoded',
      'multipart/form-data; boundary=WebAppBoundary',
      'text/plain',
      'text/html',
      'text/xml',
      'text/csv',
      'application/xml',
      'application/octet-stream',
      'application/javascript',
      'application/pdf',
    ],
  },
  {
    name: 'Accept',
    detail: '可接受的响应类型',
    values: ['application/json', 'text/html', 'text/plain', 'application/xml', '*/*'],
  },
  { name: 'Authorization', detail: '认证凭据', values: ['Bearer ', 'Basic ', 'Token ', 'Digest '] },
  { name: 'Cookie', detail: '请求 Cookie' },
  { name: 'User-Agent', detail: '客户端标识', values: ['Mozilla/5.0', 'HTTPClientPro/1.0', 'curl/8.0.0'] },
  { name: 'Accept-Encoding', detail: '可接受的编码', values: ['gzip, deflate, br', 'gzip', 'identity', '*'] },
  { name: 'Accept-Language', detail: '可接受的语言', values: ['zh-CN,zh;q=0.9,en;q=0.8', 'en-US,en;q=0.9', '*'] },
  { name: 'Accept-Charset', values: ['utf-8', 'ISO-8859-1', '*'] },
  { name: 'Cache-Control', values: ['no-cache', 'no-store', 'max-age=0', 'public', 'private', 'no-transform'] },
  { name: 'Pragma', values: ['no-cache'] },
  { name: 'Connection', values: ['keep-alive', 'close', 'upgrade'] },
  { name: 'Content-Encoding', values: ['gzip', 'deflate', 'br', 'identity'] },
  { name: 'Content-Length', detail: '请求体字节数' },
  {
    name: 'Content-Disposition',
    values: [
      'form-data; name="field"',
      'form-data; name="file"; filename="a.txt"',
      'attachment; filename="a.txt"',
    ],
  },
  { name: 'Content-Language', values: ['zh-CN', 'en-US'] },
  { name: 'Content-Range', values: ['bytes 0-99/1000'] },
  { name: 'Range', values: ['bytes=0-'] },
  { name: 'Date', detail: 'RFC 7231 日期' },
  { name: 'Expect', values: ['100-continue'] },
  { name: 'From', detail: '发起者邮箱' },
  { name: 'Host', detail: '目标主机（通常自动填充）' },
  { name: 'Origin' },
  { name: 'Referer' },
  { name: 'TE', values: ['trailers', 'gzip'] },
  { name: 'Trailer' },
  { name: 'Transfer-Encoding', values: ['chunked', 'gzip', 'identity'] },
  { name: 'Upgrade', values: ['websocket'] },
  { name: 'Via' },
  { name: 'Warning' },
  { name: 'Max-Forwards', values: ['10'] },
  { name: 'Proxy-Authorization', values: ['Basic ', 'Bearer '] },
  { name: 'If-Match', values: ['*'] },
  { name: 'If-None-Match', values: ['*'] },
  { name: 'If-Modified-Since' },
  { name: 'If-Unmodified-Since' },
  { name: 'If-Range' },
  { name: 'Sec-Fetch-Mode', values: ['cors', 'no-cors', 'navigate', 'same-origin'] },
  { name: 'Sec-Fetch-Site', values: ['cross-site', 'same-origin', 'same-site', 'none'] },
  { name: 'X-Request-Id' },
  { name: 'X-Forwarded-For' },
  { name: 'X-Forwarded-Proto', values: ['http', 'https'] },
  { name: 'X-Real-IP' },
  { name: 'X-Api-Key' },
  { name: 'X-CSRF-Token' },
  { name: 'X-Correlation-Id' },
  { name: 'DNT', values: ['1', '0'] },
];

const HEADER_INDEX = new Map<string, HeaderSpec>(
  COMMON_HEADERS.map((h) => [h.name.toLowerCase(), h]),
);

/** `$` 动态变量（由 http-core::dynamic 解析） */
export const DYNAMIC_VARIABLES: Array<{ label: string; detail: string; apply?: string }> = [
  { label: '$uuid', detail: '随机 UUID v4' },
  { label: '$random.uuid', detail: '随机 UUID v4' },
  { label: '$timestamp', detail: 'Unix 秒级时间戳' },
  { label: '$isoTimestamp', detail: 'ISO-8601 时间戳' },
  { label: '$randomInt', detail: '0~999 随机整数' },
  { label: '$random.integer', detail: '指定区间随机整数', apply: '$random.integer(0,100)' },
  { label: '$projectRoot', detail: '工程根目录' },
  { label: '$historyFolder', detail: '历史响应落盘目录' },
];

/* ------------------------------------------------------------------ */
/* 上下文识别                                                          */
/* ------------------------------------------------------------------ */

export type CompletionKind = 'method' | 'url' | 'headerName' | 'headerValue' | 'variable' | 'none';
/** 光标所处的请求块区段 */
export type Section = 'comment' | 'request-line' | 'headers' | 'body';

export interface CompletionCtx {
  kind: CompletionKind;
  /** 光标前已输入、待匹配的片段 */
  word: string;
  /** `word` 在行内的起始列（0-based） */
  wordStart: number;
  /** kind === 'headerValue' 时对应的头名（小写） */
  headerName?: string;
  /** kind === 'headerName' 时该行是否已存在冒号（决定补全后是否追加 `: `） */
  hasColon?: boolean;
}

const NONE: CompletionCtx = { kind: 'none', word: '', wordStart: 0 };

const METHOD_LINE_RE = new RegExp(`^\\s*(${METHOD_ALT})(\\s|$)`, 'i');
/** 裸 URL 请求行的起始片段 —— scheme 未输完也算（`http` / `http:` / `{{host}}`） */
const BARE_URL_START_RE = /^(https?([:/]|$)|\{\{)/i;
const REQUEST_LINE_RE = new RegExp(`^\\s*(?:(${METHOD_ALT})(?:\\s|$)|https?:\\/\\/|\\{\\{)`, 'i');
/** header 名（RFC 7230 token）+ 冒号 */
const HEADER_LINE_RE = /^([\w!#$%&'*+\-.^`|~]+)[ \t]*:/;

function isCommentLine(text: string): boolean {
  const t = text.trimStart();
  return t.startsWith('#') || t.startsWith('//');
}

function isRequestLine(text: string): boolean {
  return REQUEST_LINE_RE.test(text);
}

/** 去掉行内前导空白后的列偏移 */
function wordStartOf(before: string): number {
  return before.length - before.trimStart().length;
}

/**
 * 判定某一行所处的区段。
 * 从当前行向上找到最近的 `###` 作为块起点，再向下定位请求行与 body 边界。
 */
export function sectionAt(lines: string[], lineIdx: number): Section {
  const current = lines[lineIdx] ?? '';
  if (isCommentLine(current)) return 'comment';

  let start = 0;
  for (let i = lineIdx; i >= 0; i--) {
    if (lines[i].startsWith('###')) { start = i + 1; break; }
  }

  let reqIdx = -1;
  for (let i = start; i <= lineIdx; i++) {
    if (isRequestLine(lines[i])) { reqIdx = i; break; }
  }
  // 块内还没有请求行 —— 当前行就是请求行的位置
  if (reqIdx === -1 || reqIdx === lineIdx) return 'request-line';

  // 请求行之后：首个空行（且此前已有内容行）标记 body 开始
  let sawContent = false;
  for (let i = reqIdx + 1; i <= lineIdx; i++) {
    const text = lines[i];
    if (text.trim() === '') {
      if (sawContent && i < lineIdx) return 'body';
    } else if (!isCommentLine(text)) {
      sawContent = true;
    }
  }
  return 'headers';
}

/**
 * 识别光标处的补全上下文。
 * @param lines   文档按行切分的结果
 * @param lineIdx 0-based 行号
 * @param col     0-based 列号（光标在该行内的偏移）
 */
export function detectContextAt(lines: string[], lineIdx: number, col: number): CompletionCtx {
  const line = lines[lineIdx] ?? '';
  const before = line.slice(0, col);
  const section = sectionAt(lines, lineIdx);

  if (section === 'comment') return NONE;

  // `{{var}}` —— 任意区段都优先识别
  const braceAt = before.lastIndexOf('{{');
  if (braceAt >= 0 && !before.includes('}}', braceAt)) {
    return { kind: 'variable', word: before.slice(braceAt + 2), wordStart: braceAt + 2 };
  }

  if (section === 'body') return NONE;

  if (section === 'request-line') {
    const word = before.trimStart();
    const wordStart = wordStartOf(before);
    const m = METHOD_LINE_RE.exec(line);
    if (m) {
      const methodEnd = m[0].replace(/\s+$/, '').length;
      if (col > methodEnd) {
        // 方法之后 → URL；取最后一个空白之后的片段
        const rest = before.slice(m[0].length);
        const gap = Math.max(rest.lastIndexOf(' '), rest.lastIndexOf('\t'));
        return { kind: 'url', word: rest.slice(gap + 1), wordStart: m[0].length + gap + 1 };
      }
    } else if (/\s\S/.test(before) || BARE_URL_START_RE.test(word)) {
      // 裸 URL 请求行（`http://...` 或 `{{host}}/...`）
      return { kind: 'url', word, wordStart };
    }
    return { kind: 'method', word, wordStart };
  }

  // headers 区段
  const trimmed = line.trimStart();
  const indent = line.length - trimmed.length;
  const h = HEADER_LINE_RE.exec(trimmed);
  if (!h) {
    return { kind: 'headerName', word: before.trimStart(), wordStart: wordStartOf(before), hasColon: false };
  }
  const colonAt = indent + h[0].length - 1;
  if (col <= colonAt) {
    return { kind: 'headerName', word: before.trimStart(), wordStart: wordStartOf(before), hasColon: true };
  }
  const after = before.slice(colonAt + 1);
  return {
    kind: 'headerValue',
    word: after.trimStart(),
    wordStart: colonAt + 1 + wordStartOf(after),
    headerName: h[1].toLowerCase(),
  };
}

/* ------------------------------------------------------------------ */
/* 语料收集                                                            */
/* ------------------------------------------------------------------ */

/** 抽取 URL 的 origin 部分（`scheme://host[:port]`），无法识别时返回 null */
export function originOf(url: string): string | null {
  const m = /^[a-zA-Z][\w+.-]*:\/\/[^/?#\s]+/.exec(url);
  return m ? m[0] : null;
}

/** 从 .http 文本中抽取所有请求目标 */
export function targetsOf(text: string): string[] {
  return splitRequests(text).map((b) => b.target);
}

/** 从 .http 文本中抽取所有请求头 */
export function headersOf(text: string): Array<{ name: string; value: string }> {
  return splitRequests(text).flatMap((b) => b.headers.map((h) => ({ name: h.name, value: h.value })));
}

/** 去重并保持首次出现的顺序 */
export function dedupe(values: Iterable<string>): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const v of values) {
    if (!v || seen.has(v)) continue;
    seen.add(v);
    out.push(v);
  }
  return out;
}

/**
 * 前缀优先过滤：完全前缀匹配排在包含匹配之前，同组内保持语料原有顺序（近期优先）。
 * `word` 为空时返回全部语料。
 */
export function filterByPrefix(candidates: string[], word: string, limit = 50): string[] {
  if (!word) return candidates.slice(0, limit);
  const q = word.toLowerCase();
  const prefix: string[] = [];
  const contains: string[] = [];
  for (const c of candidates) {
    const lc = c.toLowerCase();
    if (lc.startsWith(q)) prefix.push(c);
    else if (lc.includes(q)) contains.push(c);
  }
  return [...prefix, ...contains].slice(0, limit);
}

/* ------------------------------------------------------------------ */
/* 候选项构造                                                          */
/* ------------------------------------------------------------------ */

export interface CompletionCorpus {
  /** 学习到的 URL（当前文档优先，其后是其它标签页 / 历史记录） */
  urls: string[];
  /** 学习到的 header 名 */
  headerNames: string[];
  /** 学习到的 header 值，按小写头名索引 */
  headerValues: Map<string, string[]>;
  /** 环境变量名 */
  variables: string[];
}

export const EMPTY_CORPUS: CompletionCorpus = {
  urls: [],
  headerNames: [],
  headerValues: new Map(),
  variables: [],
};

function methodOptions(word: string): Completion[] {
  const picked = filterByPrefix(METHOD_COMPLETIONS, word);
  return picked.map((label, i) => ({
    label,
    type: 'keyword',
    detail: 'method',
    boost: picked.length - i,
  }));
}

function urlOptions(word: string, corpus: CompletionCorpus): Completion[] {
  const full = corpus.urls;
  const fullSet = new Set(full);
  // host 建议 —— 便于「同一主机、换一条路径」的场景
  const origins = dedupe(
    full.map(originOf).filter((o): o is string => o !== null && !fullSet.has(o)),
  );
  const originSet = new Set(origins);
  // 排除与已输入内容完全相同的候选（正在输入的 target 本身）
  const picked = filterByPrefix(dedupe([...full, ...origins]), word).filter((l) => l !== word);
  return picked.map((label, i) => ({
    label,
    type: 'url',
    detail: originSet.has(label) ? 'host' : 'recent',
    boost: picked.length - i,
  }));
}

function headerNameOptions(ctx: CompletionCtx, corpus: CompletionCorpus): Completion[] {
  const suffix = ctx.hasColon ? '' : ': ';
  const dict = filterByPrefix(COMMON_HEADERS.map((h) => h.name), ctx.word, 60);
  const learned = filterByPrefix(dedupe(corpus.headerNames), ctx.word, 20)
    .filter((n) => !HEADER_INDEX.has(n.toLowerCase()));

  const options: Completion[] = dict.map((name, i) => ({
    label: name,
    type: 'property',
    detail: HEADER_INDEX.get(name.toLowerCase())?.detail ?? 'header',
    apply: name + suffix,
    boost: 1000 - i,
  }));
  for (const name of learned) {
    options.push({
      label: name,
      type: 'property',
      detail: 'used in this file',
      apply: name + suffix,
      boost: 10,
    });
  }
  return options;
}

function headerValueOptions(ctx: CompletionCtx, corpus: CompletionCorpus): Completion[] {
  const name = ctx.headerName ?? '';
  const spec = HEADER_INDEX.get(name);
  const learned = corpus.headerValues.get(name) ?? [];
  const candidates = dedupe([...learned, ...(spec?.values ?? [])]);
  return filterByPrefix(candidates, ctx.word).map((label, i) => ({
    label,
    type: 'text',
    detail: spec?.name ?? name,
    boost: candidates.length - i,
  }));
}

function variableOptions(word: string, corpus: CompletionCorpus): Completion[] {
  const env: Completion[] = corpus.variables.map((label) => ({
    label,
    type: 'variable',
    detail: 'environment',
    boost: 100,
  }));
  const dynamic: Completion[] = DYNAMIC_VARIABLES.map((v) => ({
    label: v.label,
    type: 'variable',
    detail: v.detail,
    apply: v.apply ?? v.label,
    boost: 50,
  }));
  const all = dedupe([...env, ...dynamic].map((o) => o.label));
  const byLabel = new Map([...env, ...dynamic].map((o) => [o.label, o]));
  const picked = word
    ? all.filter((label) => label.toLowerCase().includes(word.toLowerCase()))
    : all;
  return picked.map((label) => byLabel.get(label)!);
}

/**
 * 依据上下文构造候选项 —— 纯函数，便于单测。
 * 返回 `null` 表示此处不提供补全。
 */
export function buildOptions(ctx: CompletionCtx, corpus: CompletionCorpus = EMPTY_CORPUS): Completion[] | null {
  switch (ctx.kind) {
    case 'method': return methodOptions(ctx.word);
    case 'url': return urlOptions(ctx.word, corpus);
    case 'headerName': return headerNameOptions(ctx, corpus);
    case 'headerValue': return headerValueOptions(ctx, corpus);
    case 'variable': return variableOptions(ctx.word, corpus);
    default: return null;
  }
}

/* ------------------------------------------------------------------ */
/* CodeMirror 适配层                                                   */
/* ------------------------------------------------------------------ */

/** 由宿主（EditorPane）注入的动态语料来源 */
export interface CompletionDeps {
  /** 除当前文档外学习到的 URL（其它标签页 / 历史记录） */
  knownUrls?: () => string[];
  /** 环境变量名 */
  variableNames?: () => string[];
}

/** 从文档文本 + 注入语料构造补全语料 */
export function corpusFrom(docText: string, deps: CompletionDeps = {}): CompletionCorpus {
  const headers = headersOf(docText);
  const headerValues = new Map<string, string[]>();
  for (const h of headers) {
    const key = h.name.toLowerCase();
    const list = headerValues.get(key);
    if (list) list.push(h.value);
    else headerValues.set(key, [h.value]);
  }
  return {
    urls: dedupe([...targetsOf(docText), ...(deps.knownUrls?.() ?? [])]),
    headerNames: headers.map((h) => h.name),
    headerValues,
    variables: deps.variableNames?.() ?? [],
  };
}

/**
 * HTTP 补全扩展。
 * `activateOnTyping` 打开 —— 输入即弹出下拉；Ctrl+Space 手动触发同样可用。
 */
export function httpCompletion(deps: CompletionDeps = {}): Extension {
  const source = (context: CompletionContext): CompletionResult | null => {
    const { state, pos } = context;
    const line = state.doc.lineAt(pos);
    const docText = state.sliceDoc();
    const ctx = detectContextAt(docText.split('\n'), line.number - 1, pos - line.from);
    if (ctx.kind === 'none') return null;

    const options = buildOptions(ctx, corpusFrom(docText, deps));
    if (!options || options.length === 0) return null;

    return {
      from: line.from + ctx.wordStart,
      to: pos,
      options,
    };
  };

  return autocompletion({
    override: [source],
    activateOnTyping: true,
    defaultKeymap: true,
    icons: false,
    selectOnOpen: true,
    tooltipClass: () => 'cm-http-completion',
    optionClass: (c) => `cm-http-completion-${c.type ?? 'text'}`,
    // 选中头名后（补全为 `Name: `）继续触发取值补全
    activateOnCompletion: (c) => c.type === 'property' && (c.apply as string).endsWith(': '),
  });
}
