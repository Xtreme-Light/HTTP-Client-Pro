/**
 * curl 命令 → spec(.http) 请求格式转换。
 *
 * 粘贴检测 + 转换：转换结果中原始 curl 内容以 `#` 行注释形式置于头部（spec §2.4），
 * 其后为空行 + 请求行 + 请求头（+ 空行 + 消息体），符合 spec §3.2 语法。
 */

/** 带值选项（下一个 token 为其参数，解析时消费） */
const OPTIONS_WITH_VALUE = new Set([
  '--url', '-X', '--request', '-H', '--header', '-b', '--cookie',
  '-d', '--data', '--data-raw', '--data-binary', '--data-ascii', '--data-urlencode',
  '-A', '--user-agent', '-e', '--referer', '-u', '--user',
  '-c', '--cookie-jar', '-o', '--output', '-x', '--proxy',
  '--connect-timeout', '-m', '--max-time', '--retry', '-T', '--upload-file',
  '-F', '--form', '-w', '--write-out', '-E', '--cert', '--key', '--cacert',
  '-U', '--proxy-user', '-C', '--continue-at', '--max-filesize', '-K', '--config',
]);

/** 请求体选项 */
const DATA_OPTIONS = new Set([
  '-d', '--data', '--data-raw', '--data-binary', '--data-ascii', '--data-urlencode',
]);

/** 判断文本是否像一个 curl 命令 */
export function looksLikeCurl(text: string): boolean {
  const t = text.trim();
  if (!/^curl(\s|$)/.test(t)) return false;
  return /(?:^|[\s'"`])(--url\b|--header\b|--request\b|--data|-H\b|-X\b|-d\b|-b\b|https?:\/\/)/.test(t);
}

/**
 * 简化版 shell 分词：支持单引号、双引号（含 \" \\ 转义）与反斜杠转义，
 * 按空白切分。不展开变量。
 */
export function shellSplit(input: string): string[] {
  const tokens: string[] = [];
  let cur = '';
  let has = false; // 当前 token 是否有内容（含空串引号 ''）
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < input.length; i++) {
    const ch = input[i];
    if (inSingle) {
      if (ch === "'") inSingle = false;
      else cur += ch;
    } else if (inDouble) {
      if (ch === '"') inDouble = false;
      else if (ch === '\\' && (input[i + 1] === '"' || input[i + 1] === '\\')) cur += input[++i];
      else cur += ch;
    } else if (ch === "'") {
      inSingle = true;
      has = true;
    } else if (ch === '"') {
      inDouble = true;
      has = true;
    } else if (ch === '\\' && i + 1 < input.length) {
      cur += input[++i];
      has = true;
    } else if (/\s/.test(ch)) {
      if (has) {
        tokens.push(cur);
        cur = '';
        has = false;
      }
    } else {
      cur += ch;
      has = true;
    }
  }
  if (has) tokens.push(cur);
  return tokens;
}

/**
 * 去除值中混入的换行（粘贴来源的 curl 常有折行：引号内换行、续行残留等）。
 * 请求行与请求头按 spec §3.2 必须是单行，换行会把请求结构打散
 * （典型症状：METHOD 与 URL 被拆成两行）。
 */
function stripNewlines(s: string): string {
  return s.replace(/[\r\n]+/g, '').trim();
}

interface ParsedCurl {
  url: string;
  method?: string;
  headers: string[];
  bodyParts: string[];
}

function parseCurl(tokens: string[]): ParsedCurl {
  const parsed: ParsedCurl = { url: '', headers: [], bodyParts: [] };
  let cookie = '';
  let user = '';

  for (let i = 1; i < tokens.length; i++) {
    const tok = tokens[i];
    const nextValue = () => tokens[++i] ?? '';
    if (tok === '--url') {
      parsed.url = stripNewlines(nextValue());
    } else if (tok === '-X' || tok === '--request') {
      const m = stripNewlines(nextValue()).split(/\s+/)[0];
      if (m) parsed.method = m.toUpperCase();
    } else if (tok === '-H' || tok === '--header') {
      const h = stripNewlines(nextValue());
      if (h.includes(':')) parsed.headers.push(h);
    } else if (tok === '-b' || tok === '--cookie') {
      cookie = stripNewlines(nextValue());
    } else if (tok === '-A' || tok === '--user-agent') {
      parsed.headers.push(`User-Agent: ${stripNewlines(nextValue())}`);
    } else if (tok === '-e' || tok === '--referer') {
      parsed.headers.push(`Referer: ${stripNewlines(nextValue())}`);
    } else if (tok === '-u' || tok === '--user') {
      user = stripNewlines(nextValue());
    } else if (DATA_OPTIONS.has(tok)) {
      // body 允许保留换行（spec 消息体可为多行）
      parsed.bodyParts.push(nextValue());
    } else if (tok === '-I' || tok === '--head') {
      parsed.method ??= 'HEAD';
    } else if (OPTIONS_WITH_VALUE.has(tok)) {
      nextValue(); // 已知的其他带值选项 — 忽略其参数
    } else if (tok.startsWith('-')) {
      // 未知选项 — 忽略（布尔标志或未知选项）
    } else if (!parsed.url) {
      parsed.url = stripNewlines(tok); // 裸 URL 位置参数
    }
  }

  if (cookie) parsed.headers.push(`Cookie: ${cookie}`);
  if (user) {
    try {
      parsed.headers.push(`Authorization: Basic ${btoa(user)}`);
    } catch {
      // 非 Latin-1 凭据无法 base64 编码 — 跳过
    }
  }
  // 去除 URL 外围的引号或反斜杠转义残留的反引号
  parsed.url = parsed.url.replace(/^["'`]+|["'`]+$/g, '');
  return parsed;
}

/**
 * 将 curl 命令转换为 spec 请求块文本（不含 ### 分隔符，由调用方按上下文决定）。
 * 输出结构：
 * ```
 * # <原始 curl 第 1 行>
 * # <原始 curl 第 2 行>
 * ...
 * <空行>
 * METHOD URL
 * Header: value
 * ...
 * <空行（仅有 body 时）>
 * <body>
 * ```
 */
export function curlToHttp(text: string): string {
  const normalized = text.replace(/\r\n?/g, '\n');
  // 原始内容逐行转为 # 注释（spec §2.4）
  const comment = normalized
    .trimEnd()
    .split('\n')
    .map((l) => `# ${l.trimEnd()}`.trimEnd())
    .join('\n');

  // 行尾续行符 `\` 拼接后分词
  const tokens = shellSplit(normalized.replace(/\\\n/g, ' '));
  if (tokens.length === 0 || tokens[0] !== 'curl') return comment;

  const parsed = parseCurl(tokens);
  if (!parsed.url) return comment; // 无法提取 URL — 仅保留注释

  const method = parsed.method ?? (parsed.bodyParts.length > 0 ? 'POST' : 'GET');
  const lines: string[] = [comment, '', `${method} ${parsed.url}`, ...parsed.headers];
  if (parsed.bodyParts.length > 0) {
    lines.push('');
    // curl 语义：多个 -d 以 & 连接
    lines.push(parsed.bodyParts.join('&'));
  }
  return lines.join('\n');
}

/**
 * 按粘贴位置上下文计算最终插入文本：
 * - 光标前已有内容且最后一非空行不是 `###` 分隔行时，自动补 `###` 分隔符（spec §2.5/§3.1），
 *   避免转换出的请求与已有块合并；
 * - 光标后仍有内容时补换行，避免与后续内容粘连。
 *
 * @param before 光标前的文档文本
 * @param after  光标（选区）后的文档文本
 * @param block  curlToHttp 生成的请求块
 */
export function composeCurlPasteInsertion(before: string, after: string, block: string): string {
  const lastNonEmpty = before.split('\n').reverse().find((l) => l.trim() !== '') ?? '';
  let prefix = '';
  if (lastNonEmpty.startsWith('###')) {
    // 紧跟 ### 分隔行 — 附加到该分隔符（必要时先换行），不重复添加
    prefix = before.endsWith('\n') ? '' : '\n';
  } else if (before.trim() !== '') {
    prefix = before.endsWith('\n') ? '###\n' : '\n###\n';
  }
  const suffix = after.trim() !== '' ? '\n' : '';
  return prefix + block + suffix;
}
