/**
 * CodeMirror 6 StreamLanguage for .http / .rest files.
 *
 * Token types emitted:
 * - `separator`        — `###` prefix of the first separator in a block
 * - `separatorName`    — name text after the first `###` of a block (request title)
 * - `separatorComment` — subsequent consecutive `###` lines (spec §2.5/§3.1 允许多个连续分隔符，
 *                        视觉上作为普通注释展示)
 * - `comment`          — `#` or `//` lines
 * - `method`           — HTTP method keyword (GET, POST, ...)
 * - `url`              — request target URL
 * - `headerName`       — header field name before `:`
 * - `headerValue`      — header value after `:`
 * - `variable`         — `{{var}}` references
 * - `body`             — body section (after first blank line)
 */

import { StreamLanguage } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

/** 使用标准 Lezer tags 避免 "Unknown highlighting tag" 警告 */
export const httpTags = {
  separator: t.contentSeparator,
  separatorName: t.heading,
  separatorComment: t.lineComment,
  comment: t.comment,
  method: t.keyword,
  url: t.url,
  headerName: t.propertyName,
  headerValue: t.string,
  variable: t.variableName,
  body: t.content,
};

const HTTP_METHODS = ['GET', 'HEAD', 'POST', 'PUT', 'DELETE', 'CONNECT', 'PATCH', 'OPTIONS', 'TRACE'];
const METHOD_RE = new RegExp(`^(${HTTP_METHODS.join('|')})\\b`, 'i');
const VAR_RE = /\{\{[^}]*\}\}/;

interface StreamState {
  inBody: boolean;
  seenRequestLine: boolean;
  /** 上一个非空行是否为 ### 分隔行（用于识别连续分隔符序列） */
  prevSeparator: boolean;
  /** 当前行剩余部分是否为分隔行名称文本 */
  sepNameRest: boolean;
  /** 当前行是否为 header 行（行内剩余部分按 headerValue 处理） */
  inHeader: boolean;
}

export const httpLanguage = StreamLanguage.define<StreamState>({
  name: 'http',
  startState: () => ({ inBody: false, seenRequestLine: false, prevSeparator: false, sepNameRest: false, inHeader: false }),
  token(stream, state) {
    // 逐行处理
    if (stream.sol()) {
      // 进入新行 — 清除裸 ### 行（无名称文本）可能残留的状态
      state.sepNameRest = false;
      state.inHeader = false;
      const line = stream.string;

      // ### separator
      if (line.startsWith('###')) {
        state.inBody = false;
        state.seenRequestLine = false;
        if (state.prevSeparator) {
          // 连续分隔符序列中的后续分隔行 — 作为普通注释展示
          stream.skipToEnd();
          state.prevSeparator = true;
          return 'separatorComment';
        }
        // 序列中的首个分隔行 — ### 前缀 + 名称文本（块标题）
        stream.match(/^###\s*/);
        state.prevSeparator = true;
        state.sepNameRest = true;
        return 'separator';
      }

      // 空行 — body 开始的边界（不打断分隔符序列）
      if (line.trim() === '') {
        if (state.seenRequestLine) {
          state.inBody = true;
        }
        stream.skipToEnd();
        return null;
      }

      // 其他内容行结束分隔符序列
      state.prevSeparator = false;

      // # or // comment
      if (line.startsWith('#') || line.trim().startsWith('//')) {
        stream.skipToEnd();
        return 'comment';
      }

      // body 区域
      if (state.inBody) {
        // 仍然处理 {{var}}
        if (stream.match(/^\{\{[^}]*\}\}/)) return 'variable';
        // 跳过到下一个 {{ 或行尾
        const m = stream.match(/^[^{]+/);
        if (!m) stream.next();
        return 'body';
      }

      // 请求行 (method + url)
      if (!state.seenRequestLine && METHOD_RE.test(line.trim())) {
        // 匹配 method
        const m = stream.match(new RegExp(`^\\s*(${HTTP_METHODS.join('|')})`, 'i'));
        if (m) {
          state.seenRequestLine = true;
          return 'method';
        }
      }

      // header line: name: value
      const headerMatch = stream.match(/^\s*([\w-]+)\s*:/);
      if (headerMatch) {
        state.inHeader = true;
        return 'headerName';
      }

      // 未识别的行 — 如果 seenRequestLine 则当作 header 处理
      if (state.seenRequestLine) {
        // 尝试匹配 {{var}}
        if (stream.match(/^\{\{[^}]*\}\}/)) return 'variable';
        stream.skipToEnd();
        return 'headerValue';
      }

      // 请求行 URL 部分（method 已匹配后的剩余）
      if (stream.match(/^\s*\S+/)) {
        // 检查是否含 {{var}}
        // 处理 URL 中的 {{var}} — 逐 token 匹配
        return 'url';
      }

      stream.skipToEnd();
      return null;
    }

    // 分隔行名称文本（### 前缀匹配后的行内剩余部分）
    if (state.sepNameRest) {
      stream.skipToEnd();
      state.sepNameRest = false;
      return 'separatorName';
    }

    // 行内继续处理：匹配 {{var}} 或剩余文本
    if (stream.match(/^\{\{[^}]*\}\}/)) return 'variable';

    // header value 行内（headerName 之后的剩余部分）
    if (state.inHeader) {
      const m = stream.match(/^[^{]+/);
      if (!m) stream.next();
      return 'headerValue';
    }

    // 在 URL 行中
    if (state.seenRequestLine && !state.inBody) {
      const m = stream.match(/^[^{]+/);
      if (!m) stream.next();
      return 'url';
    }

    // body 行内
    if (state.inBody) {
      const m = stream.match(/^[^{]+/);
      if (!m) stream.next();
      return 'body';
    }

    stream.eatSpace();
    return null;
  },
  languageData: {
    commentTokens: { line: '#' },
  },
  // StreamLanguage 的 tokenTable 是「token 名 → Tag」的普通对象（作为 TokenTable.extra），
  // 不能传 styleTags() 的返回值（那是 Lezer NodePropSource，不会生效）。
  tokenTable: {
    separator: t.contentSeparator,
    separatorName: t.heading,
    separatorComment: t.lineComment,
    comment: t.comment,
    method: t.keyword,
    url: t.url,
    headerName: t.propertyName,
    headerValue: t.string,
    variable: t.variableName,
    body: t.content,
  },
});
