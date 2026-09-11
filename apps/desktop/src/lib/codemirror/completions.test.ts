import { describe, it, expect } from 'vitest';
import { EditorView } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { acceptCompletion, completionStatus, startCompletion } from '@codemirror/autocomplete';
import {
  COMMON_HEADERS,
  METHOD_COMPLETIONS,
  buildOptions,
  corpusFrom,
  dedupe,
  detectContextAt,
  filterByPrefix,
  headersOf,
  httpCompletion,
  originOf,
  sectionAt,
  targetsOf,
  type CompletionCorpus,
  type CompletionDeps,
} from './completions';

/** 把多行文本按行切分（与编辑器文档保持一致） */
const L = (...lines: string[]) => lines;

const DOC = [
  '### 登录',
  'POST http://10.70.80.1:9003/api/login',
  'Content-Type: application/json',
  'X-Tenant: acme',
  '',
  '{ "user": "admin" }',
  '',
  '### 获取数据',
  'GET https://api.example.com/v1/users?page=1',
  'Accept: application/json',
  'Authorization: Bearer {{token}}',
].join('\n');

function ctxOf(doc: string, lineIdx: number, col: number) {
  return detectContextAt(doc.split('\n'), lineIdx, col);
}

/* ------------------------------------------------------------------ */

describe('sectionAt', () => {
  const lines = DOC.split('\n');

  it('识别注释 / 分隔符行', () => {
    expect(sectionAt(lines, 0)).toBe('comment');
    expect(sectionAt(L('# 说明'), 0)).toBe('comment');
    expect(sectionAt(L('// 说明'), 0)).toBe('comment');
  });

  it('块内还没有请求行时视为请求行区段', () => {
    expect(sectionAt(L('### 名称', ''), 1)).toBe('request-line');
    expect(sectionAt(L(''), 0)).toBe('request-line');
    expect(sectionAt(L('### 名称', 'POST http://x'), 1)).toBe('request-line');
  });

  it('请求行之后、空行之前是 header 区段', () => {
    expect(sectionAt(lines, 2)).toBe('headers');
    expect(sectionAt(lines, 3)).toBe('headers');
  });

  it('空行之后的内容属于 body', () => {
    expect(sectionAt(lines, 5)).toBe('body');
  });

  it('新的 ### 块重新从请求行开始', () => {
    expect(sectionAt(lines, 8)).toBe('request-line');
    expect(sectionAt(lines, 9)).toBe('headers');
  });
});

describe('detectContextAt', () => {
  it('空行 / 方法词内 → method', () => {
    expect(ctxOf('GET http://x', 0, 0)).toMatchObject({ kind: 'method', word: '', wordStart: 0 });
    expect(ctxOf('GE', 0, 2)).toMatchObject({ kind: 'method', word: 'GE', wordStart: 0 });
    expect(ctxOf('GET', 0, 3)).toMatchObject({ kind: 'method', word: 'GET' });
    expect(ctxOf('### a\npos', 1, 3)).toMatchObject({ kind: 'method', word: 'pos' });
  });

  it('方法之后 → url，并给出正确的匹配起点', () => {
    const c = ctxOf('GET http', 0, 8);
    expect(c).toMatchObject({ kind: 'url', word: 'http', wordStart: 4 });
    expect(ctxOf('POST http://10.70.80.1:9003/api', 0, 31))
      .toMatchObject({ kind: 'url', word: 'http://10.70.80.1:9003/api', wordStart: 5 });
  });

  it('裸 URL 请求行也补全 url', () => {
    expect(ctxOf('http://10.70', 0, 12)).toMatchObject({ kind: 'url', word: 'http://10.70', wordStart: 0 });
  });

  it('scheme 尚未输完时即给出 URL 建议', () => {
    expect(ctxOf('http', 0, 4)).toMatchObject({ kind: 'url', word: 'http', wordStart: 0 });
    expect(ctxOf('http:', 0, 5)).toMatchObject({ kind: 'url', word: 'http:' });
  });

  it('header 名位置 → headerName，无冒号时补全后追加 ": "', () => {
    expect(ctxOf('GET http://x\nCont', 1, 4))
      .toMatchObject({ kind: 'headerName', word: 'Cont', wordStart: 0, hasColon: false });
    expect(ctxOf('GET http://x\nConte', 1, 5))
      .toMatchObject({ kind: 'headerName', word: 'Conte', hasColon: false });
  });

  it('已有冒号时 headerName 不再追加 ": "', () => {
    expect(ctxOf('GET http://x\nCont: v', 1, 4))
      .toMatchObject({ kind: 'headerName', word: 'Cont', hasColon: true });
  });

  it('冒号之后 → headerValue，并带出小写头名', () => {
    const c = ctxOf('GET http://x\nContent-Type: application/js', 1, 28);
    expect(c).toMatchObject({ kind: 'headerValue', headerName: 'content-type', word: 'application/js' });
    expect(c.wordStart).toBe(14);
  });

  it('刚敲完冒号 + 空格时 word 为空', () => {
    const c = ctxOf('GET http://x\nContent-Type: ', 1, 14);
    expect(c).toMatchObject({ kind: 'headerValue', word: '', wordStart: 14 });
  });

  it('{{ 内 → variable（任意区段）', () => {
    expect(ctxOf('GET {{ho', 0, 8)).toMatchObject({ kind: 'variable', word: 'ho', wordStart: 6 });
    expect(ctxOf('GET http://x\nAuthorization: Bearer {{tok', 1, 27))
      .toMatchObject({ kind: 'variable', word: 'tok' });
    expect(ctxOf('GET http://x\nA: b\n\n{"k": "{{va', 3, 13))
      .toMatchObject({ kind: 'variable', word: 'va' });
  });

  it('已闭合的 {{var}} 不再触发变量补全', () => {
    const c = ctxOf('GET {{host}}/ap', 0, 15);
    expect(c.kind).toBe('url');
  });

  it('注释行与 body 不补全', () => {
    expect(ctxOf('# GET http', 0, 5).kind).toBe('none');
    expect(ctxOf('### 名称', 0, 4).kind).toBe('none');
    expect(ctxOf(DOC, 5, 5).kind).toBe('none');
  });
});

/* ------------------------------------------------------------------ */

describe('originOf', () => {
  it('抽取 scheme://host[:port]', () => {
    expect(originOf('http://10.70.80.1:9003/api/login')).toBe('http://10.70.80.1:9003');
    expect(originOf('https://api.example.com/v1')).toBe('https://api.example.com');
    expect(originOf('http://localhost:8080')).toBe('http://localhost:8080');
  });

  it('非 URL 返回 null', () => {
    expect(originOf('/api/users')).toBeNull();
    expect(originOf('{{host}}/api')).toBeNull();
  });
});

describe('targetsOf / headersOf', () => {
  it('从文档收集请求目标与请求头', () => {
    expect(targetsOf(DOC)).toEqual([
      'http://10.70.80.1:9003/api/login',
      'https://api.example.com/v1/users?page=1',
    ]);
    expect(headersOf(DOC)).toEqual([
      { name: 'Content-Type', value: 'application/json' },
      { name: 'X-Tenant', value: 'acme' },
      { name: 'Accept', value: 'application/json' },
      { name: 'Authorization', value: 'Bearer {{token}}' },
    ]);
  });

  it('空行之后的内容归入 body，不会被当成 header', () => {
    expect(headersOf('GET http://x\nX-A: 1\n\nfoo: bar\n')).toEqual([{ name: 'X-A', value: '1' }]);
  });
});

describe('dedupe / filterByPrefix', () => {
  it('去重且保持首次出现顺序，丢弃空串', () => {
    expect(dedupe(['a', 'b', 'a', '', 'c'])).toEqual(['a', 'b', 'c']);
  });

  it('前缀匹配优先于包含匹配', () => {
    expect(filterByPrefix(['xabc', 'abc', 'zab'], 'ab')).toEqual(['abc', 'xabc', 'zab']);
  });

  it('空 word 返回全部并受 limit 约束', () => {
    expect(filterByPrefix(['a', 'b', 'c'], '')).toEqual(['a', 'b', 'c']);
    expect(filterByPrefix(['a', 'b', 'c'], '', 2)).toEqual(['a', 'b']);
  });

  it('大小写不敏感', () => {
    expect(filterByPrefix(['Application/JSON'], 'app')).toEqual(['Application/JSON']);
  });
});

/* ------------------------------------------------------------------ */

describe('buildOptions — method', () => {
  it('输入前缀给出匹配的方法，GET 排最前', () => {
    const labels = buildOptions({ kind: 'method', word: '', wordStart: 0 })!.map((o) => o.label);
    expect(labels).toEqual(METHOD_COMPLETIONS);
    expect(buildOptions({ kind: 'method', word: 'po', wordStart: 0 })!.map((o) => o.label)).toEqual(['POST']);
  });
});

describe('buildOptions — url', () => {
  const corpus: CompletionCorpus = {
    ...corpusFrom(DOC),
    urls: [...corpusFrom(DOC).urls, 'http://10.70.80.1:9003/api/health'],
  };

  it('输入 http 即可看到文档里出现过的完整 URL', () => {
    const labels = buildOptions({ kind: 'url', word: 'http', wordStart: 4 }, corpus)!.map((o) => o.label);
    expect(labels).toContain('http://10.70.80.1:9003/api/login');
    expect(labels).toContain('https://api.example.com/v1/users?page=1');
  });

  it('按输入前缀收窄', () => {
    const labels = buildOptions({ kind: 'url', word: 'http://10.70', wordStart: 4 }, corpus)!.map((o) => o.label);
    expect(labels.every((l) => l.startsWith('http://10.70'))).toBe(true);
    expect(labels).not.toContain('https://api.example.com/v1/users?page=1');
  });

  it('额外给出 host 建议，便于同主机换路径', () => {
    const options = buildOptions({ kind: 'url', word: 'http://10', wordStart: 4 }, corpus)!;
    const host = options.find((o) => o.label === 'http://10.70.80.1:9003');
    expect(host).toBeDefined();
    expect(host!.detail).toBe('host');
  });

  it('没有语料时不给出候选', () => {
    expect(buildOptions({ kind: 'url', word: 'http', wordStart: 4 }, corpusFrom(''))).toEqual([]);
  });
});

describe('buildOptions — headerName', () => {
  it('常见头优先，且无冒号时补全后追加 ": "', () => {
    const options = buildOptions({ kind: 'headerName', word: 'cont', wordStart: 0, hasColon: false }, corpusFrom(DOC))!;
    const labels = options.map((o) => o.label);
    expect(labels[0]).toBe('Content-Type');
    expect(labels).toContain('Content-Length');
    expect(options[0].apply).toBe('Content-Type: ');
  });

  it('已有冒号时只补全名字', () => {
    const options = buildOptions({ kind: 'headerName', word: 'Cont', wordStart: 0, hasColon: true }, corpusFrom(DOC))!;
    expect(options[0].apply).toBe('Content-Type');
  });

  it('文档里用过的自定义头也会被学习', () => {
    const options = buildOptions({ kind: 'headerName', word: 'X-Ten', wordStart: 0, hasColon: false }, corpusFrom(DOC))!;
    expect(options.map((o) => o.label)).toContain('X-Tenant');
    expect(options.find((o) => o.label === 'X-Tenant')!.detail).toBe('used in this file');
  });

  it('词典里的所有头名唯一', () => {
    const names = COMMON_HEADERS.map((h) => h.name.toLowerCase());
    expect(new Set(names).size).toBe(names.length);
  });
});

describe('buildOptions — headerValue', () => {
  it('Content-Type 给出 application/json', () => {
    const labels = buildOptions(
      { kind: 'headerValue', word: '', wordStart: 14, headerName: 'content-type' },
      corpusFrom(DOC),
    )!.map((o) => o.label);
    expect(labels).toContain('application/json');
    expect(labels).toContain('application/x-www-form-urlencoded');
  });

  it('按已输入内容过滤（前缀优先，其后是包含匹配）', () => {
    const prefix = buildOptions(
      { kind: 'headerValue', word: 'text', wordStart: 14, headerName: 'content-type' },
      corpusFrom(''),
    )!.map((o) => o.label);
    expect(prefix).toEqual(['text/plain', 'text/html', 'text/xml', 'text/csv']);

    const contains = buildOptions(
      { kind: 'headerValue', word: 'json', wordStart: 14, headerName: 'content-type' },
      corpusFrom(''),
    )!.map((o) => o.label);
    expect(contains).toEqual(['application/json']);
  });

  it('学习当前文档里同名的取值', () => {
    const labels = buildOptions(
      { kind: 'headerValue', word: '', wordStart: 11, headerName: 'x-tenant' },
      corpusFrom(DOC),
    )!.map((o) => o.label);
    expect(labels).toContain('acme');
  });

  it('Authorization 给出 Bearer / Basic', () => {
    const labels = buildOptions(
      { kind: 'headerValue', word: 'Bea', wordStart: 15, headerName: 'authorization' },
      corpusFrom(''),
    )!.map((o) => o.label);
    expect(labels).toContain('Bearer ');
  });

  it('未知头名且无语料时不给出候选', () => {
    expect(buildOptions({ kind: 'headerValue', word: '', wordStart: 3, headerName: 'x-unknown' }, corpusFrom(''))).toEqual([]);
  });
});

describe('buildOptions — variable', () => {
  it('给出环境变量与 $ 动态变量', () => {
    const corpus = corpusFrom(DOC, { variableNames: () => ['token', 'host', 'userId'] });
    const labels = buildOptions({ kind: 'variable', word: '', wordStart: 6 }, corpus)!.map((o) => o.label);
    expect(labels).toContain('token');
    expect(labels).toContain('$uuid');
    expect(labels).toContain('$timestamp');
  });

  it('按输入过滤，$random.integer 补全为带参形式', () => {
    const corpus = corpusFrom(DOC, { variableNames: () => ['token'] });
    const options = buildOptions({ kind: 'variable', word: '$random.i', wordStart: 6 }, corpus)!;
    expect(options.map((o) => o.label)).toEqual(['$random.integer']);
    expect(options[0].apply).toBe('$random.integer(0,100)');
  });
});

describe('corpusFrom', () => {
  it('合并文档 URL 与注入的外部 URL，并去重', () => {
    const corpus = corpusFrom(DOC, { knownUrls: () => ['http://10.70.80.1:9003/api/login', 'http://other:1/x'] });
    expect(corpus.urls).toEqual([
      'http://10.70.80.1:9003/api/login',
      'https://api.example.com/v1/users?page=1',
      'http://other:1/x',
    ]);
  });

  it('按小写头名索引取值', () => {
    const corpus = corpusFrom(DOC);
    expect(corpus.headerValues.get('content-type')).toEqual(['application/json']);
    expect(corpus.headerValues.get('accept')).toEqual(['application/json']);
  });
});

describe('buildOptions — none', () => {
  it('注释与 body 区段不提供补全', () => {
    expect(buildOptions({ kind: 'none', word: '', wordStart: 0 })).toBeNull();
  });
});

/* ------------------------------------------------------------------ */
/* CodeMirror 适配层                                                   */
/* ------------------------------------------------------------------ */

/** 挂载一个只装了补全扩展的编辑器，返回清理函数 */
function mountEditor(doc: string, pos: number, deps: CompletionDeps = {}) {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({
      doc,
      selection: { anchor: pos },
      extensions: [httpCompletion(deps)],
    }),
    parent,
  });
  return { view, cleanup: () => { view.destroy(); parent.remove(); } };
}

/** 打开下拉并等待查询完成（CodeMirror 对补全查询有 50ms 去抖） */
async function openDropdown(view: EditorView) {
  startCompletion(view);
  await new Promise((resolve) => setTimeout(resolve, 80));
}

function visibleLabels(view: EditorView): string[] {
  return [...view.dom.querySelectorAll('.cm-tooltip-autocomplete li .cm-completionLabel')]
    .map((el) => el.textContent ?? '');
}

describe('httpCompletion 适配层', () => {
  it('输入 http 即弹出下拉，列出文档里出现过的 URL 与 host', async () => {
    const { view, cleanup } = mountEditor(
      'POST http://10.70.80.1:9003/api/login\n\n### 2\nGET http',
      53,
    );
    try {
      await openDropdown(view);
      expect(completionStatus(view.state)).toBe('active');
      const labels = visibleLabels(view);
      expect(labels).toContain('http://10.70.80.1:9003/api/login');
      expect(labels).toContain('http://10.70.80.1:9003');
      // 正在输入的片段本身不作为候选回显
      expect(labels).not.toContain('http');
    } finally {
      cleanup();
    }
  });

  it('选中头名后自动追加 ": "', async () => {
    const { view, cleanup } = mountEditor('GET http://x\nCont', 17);
    try {
      await openDropdown(view);
      const labels = visibleLabels(view);
      expect(labels[0]).toBe('Content-Type');
      // acceptCompletion 有 interactionDelay（默认 75ms）保护：下拉打开后需再等一会才接受
      await new Promise((resolve) => setTimeout(resolve, 80));
      expect(acceptCompletion(view)).toBe(true);
      expect(view.state.doc.toString()).toBe(`GET http://x\n${labels[0]}: `);
    } finally {
      cleanup();
    }
  });

  it('{{ 内提示注入的环境变量', async () => {
    const { view, cleanup } = mountEditor('GET http://x\nA: {{tok', 21, {
      variableNames: () => ['token', 'host'],
    });
    try {
      await openDropdown(view);
      expect(visibleLabels(view)).toContain('token');
    } finally {
      cleanup();
    }
  });

  it('body 区段不弹出下拉', async () => {
    const { view, cleanup } = mountEditor('GET http://x\nA: 1\n\n{"k": ', 25);
    try {
      await openDropdown(view);
      expect(completionStatus(view.state)).toBeNull();
    } finally {
      cleanup();
    }
  });
});
