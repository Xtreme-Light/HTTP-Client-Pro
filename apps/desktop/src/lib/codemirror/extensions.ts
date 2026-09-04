/**
 * CodeMirror 6 高亮样式 + {{var}} 装饰插件 + 请求块卡片化。
 */

import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { ViewPlugin, Decoration, type DecorationSet, EditorView, WidgetType, gutter, GutterMarker, type ViewUpdate } from '@codemirror/view';
import { EditorState, Range, RangeSet } from '@codemirror/state';
import { httpTags, httpLanguage } from './lang-http';
import { looksLikeCurl, curlToHttp, composeCurlPasteInsertion } from './curl-to-http';
import { getRunStatus, type RunStatus } from './run-status';

/** 高亮样式 — 每个 tag 对应一个 CSS class */
export const httpHighlightStyle = HighlightStyle.define([
  { tag: httpTags.separator, class: 'cm-separator' },
  { tag: httpTags.separatorName, class: 'cm-separator-name' },
  { tag: httpTags.separatorComment, class: 'cm-separator-comment' },
  { tag: httpTags.comment, class: 'cm-comment' },
  { tag: httpTags.method, class: 'cm-method' },
  { tag: httpTags.url, class: 'cm-url' },
  { tag: httpTags.headerName, class: 'cm-header-name' },
  { tag: httpTags.headerValue, class: 'cm-header-value' },
  { tag: httpTags.variable, class: 'cm-var' },
  { tag: httpTags.body, class: 'cm-body' },
]);

const VAR_RE = /\{\{\s*([\w.-]+)\s*\}\}/g;

/**
 * 变量装饰插件 — 在 {{var}} 上添加 Decoration.mark。
 * `checkDefined` 回调决定变量是否已定义（连接 environment store）。
 */
export function variableDecoration(checkDefined: (name: string) => boolean) {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = this.build(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          this.decorations = this.build(update.view);
        }
      }

      build(view: EditorView): DecorationSet {
        const decorations: Range<Decoration>[] = [];
        for (const { from, to } of view.visibleRanges) {
          const text = view.state.sliceDoc(from, to);
          let m: RegExpExecArray | null;
          VAR_RE.lastIndex = 0;
          while ((m = VAR_RE.exec(text)) !== null) {
            const name = m[1];
            const defined = checkDefined(name);
            decorations.push(
              Decoration.mark({
                class: defined ? 'cm-var-defined' : 'cm-var-undefined',
                attributes: { title: defined ? `{{${name}}} = defined` : `{{${name}}} = undefined` },
              }).range(from + m.index, from + m.index + m[0].length),
            );
          }
        }
        return Decoration.set(decorations, true);
      }
    },
    {
      decorations: (v) => v.decorations,
    },
  );
}

/**
 * Run 按钮 GutterMarker — 在 gutter 中显示绿色 ▶ 按钮。
 * 点击时通过 dispatchEvent 触发执行（bubbles: true 确保 EditorPane 能监听到）。
 * 根据执行状态在按钮右下角叠加徽标：执行中转圈动画，完成后绿√，失败红✕。
 */
class RunButtonMarker extends GutterMarker {
  constructor(readonly line: number, readonly status: RunStatus | undefined) { super(); }

  toDOM(view: EditorView): HTMLElement {
    const btn = document.createElement('div');
    btn.className = 'cm-run-btn';
    btn.textContent = '▶';
    btn.title = 'Run this request';

    if (this.status) {
      const badge = document.createElement('span');
      badge.className = `cm-run-badge ${this.status}`;
      badge.textContent = this.status === 'done' ? '✓' : this.status === 'error' ? '✕' : '';
      badge.title = this.status === 'running' ? 'Running' : this.status === 'done' ? 'Finished' : 'Failed';
      btn.appendChild(badge);
    }

    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      // bubbles: true 让事件冒泡到 editorEl，使 EditorPane 的监听器能捕获
      view.dom.dispatchEvent(new CustomEvent('cm-run-block', { detail: { line: this.line }, bubbles: true }));
    });
    return btn;
  }
}

/**
 * 请求块卡片化插件 — 在 ### 行添加区块边框装饰，在请求行添加行样式。
 * Run 按钮不再放在编辑区域，而是通过 runGutter() 在独立 gutter 列中显示。
 */
export function blockDecoration() {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = this.build(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          this.decorations = this.build(update.view);
        }
      }

      build(view: EditorView): DecorationSet {
        const decorations: Range<Decoration>[] = [];
        const lineCount = view.state.doc.lines;

        for (let i = 1; i <= lineCount; i++) {
          const line = view.state.doc.line(i);
          const text = line.text;

          // ### separator — 仅序列中的首个分隔行添加区块起始边框，
          // 避免连续分隔符（名称 + 描述行）在视觉上被拆开
          if (text.startsWith('###')) {
            let firstInRun = true;
            for (let j = i - 1; j >= 1; j--) {
              const prev = view.state.doc.line(j).text;
              if (prev.trim() === '') continue;
              if (prev.startsWith('###')) firstInRun = false;
              break;
            }
            if (firstInRun) {
              decorations.push(
                Decoration.line({ class: 'cm-block-start' }).range(line.from),
              );
            }
          }

          // 请求行 (method + url) — 添加行样式（不再添加 widget 到编辑区域）
          const methodMatch = text.match(/^\s*(GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE)\s+/i);
          if (methodMatch) {
            decorations.push(
              Decoration.line({ class: 'cm-request-line' }).range(line.from),
            );
          }
        }

        return Decoration.set(decorations, true);
      }
    },
    {
      decorations: (v) => v.decorations,
    },
  );
}

/**
 * Run 按钮 gutter — 独立的 gutter 列，在请求行显示绿色 ▶ 按钮。
 * 这模仿 JetBrains 风格：gutter 区域可以包含行号、执行按钮等，不占据编辑区域。
 * 状态变化时由 EditorPane 通过 compartment 重配置触发 markers 重建。
 */
export function runGutter() {
  return gutter({
    class: 'cm-run-gutter',
    markers: (view) => {
      const markers: Range<RunButtonMarker>[] = [];
      const lineCount = view.state.doc.lines;
      for (let i = 1; i <= lineCount; i++) {
        const text = view.state.doc.line(i).text;
        if (/^\s*(GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE)\s+/i.test(text)) {
          markers.push(new RunButtonMarker(i, getRunStatus(i)).range(view.state.doc.line(i).from));
        }
      }
      return RangeSet.of(markers, true);
    },
  });
}

/**
 * 当前激活块的装饰插件 — 高亮当前选中的请求块（JetBrains 风格左侧绿线）。
 * 直接从编辑器光标位置计算当前块，不依赖外部 store，确保同步更新。
 */
export function activeBlockDecoration() {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = this.build(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged || update.selectionSet) {
          this.decorations = this.build(update.view);
        }
      }

      build(view: EditorView): DecorationSet {
        const decorations: Range<Decoration>[] = [];
        const lineCount = view.state.doc.lines;
        if (lineCount === 0) return Decoration.none;

        // 当前光标所在行（1-based）
        const cursorPos = view.state.selection.main.head;
        const cursorLine = view.state.doc.lineAt(cursorPos).number;

        // 从光标行向上找 ### 分隔符（块起始）
        let startLine = 1;
        for (let i = cursorLine; i >= 1; i--) {
          if (view.state.doc.line(i).text.startsWith('###')) {
            startLine = i;
            break;
          }
        }

        // 向上延伸至连续分隔符序列的首行（名称行），
        // 确保「名称 + 描述」多行分隔符整体归属当前块
        for (let i = startLine - 1; i >= 1; i--) {
          const text = view.state.doc.line(i).text;
          if (text.trim() === '') continue;
          if (!text.startsWith('###')) break;
          startLine = i;
        }

        // 从 startLine 向下找下一个 ### 或 EOF（块结束）
        // 属于块首分隔符序列的 ### 行不视为块结束
        let endLine = lineCount;
        for (let i = startLine + 1; i <= lineCount; i++) {
          if (view.state.doc.line(i).text.startsWith('###')) {
            let inStartRun = true;
            for (let j = i - 1; j >= startLine; j--) {
              const prev = view.state.doc.line(j).text;
              if (prev.trim() === '') continue;
              if (!prev.startsWith('###')) inStartRun = false;
              break;
            }
            if (!inStartRun) {
              endLine = i - 1;
              break;
            }
          }
        }

        // 验证该块确实包含一个请求行（避免高亮纯注释块）
        let hasRequest = false;
        for (let i = startLine; i <= endLine; i++) {
          if (/^\s*(GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE)\s+/i.test(view.state.doc.line(i).text)) {
            hasRequest = true;
            break;
          }
        }
        if (!hasRequest) return Decoration.none;

        for (let i = startLine; i <= endLine; i++) {
          const line = view.state.doc.line(i);
          decorations.push(
            Decoration.line({ class: 'cm-active-block' }).range(line.from),
          );
        }

        return Decoration.set(decorations, true);
      }
    },
    {
      decorations: (v) => v.decorations,
    },
  );
}

/**
 * curl 粘贴转换 — 剪贴板内容为 curl 命令时，转换为 spec 请求格式后插入，
 * 原始 curl 内容以 `#` 注释保留在头部（见 curl-to-http.ts）。
 * 按上下文自动补 `###` 分隔符，避免与已有请求块合并。
 */
export function curlPasteSupport() {
  return EditorView.domEventHandlers({
    paste(event, view) {
      const text = event.clipboardData?.getData('text/plain');
      if (!text || !looksLikeCurl(text)) return false;

      const block = curlToHttp(text);
      const { state } = view;
      const { from, to } = state.selection.main;
      const before = state.doc.sliceString(0, from);
      const after = state.doc.sliceString(to);
      const insert = composeCurlPasteInsertion(before, after, block);

      view.dispatch({
        ...state.replaceSelection(insert),
        userEvent: 'input.paste',
      });
      return true;
    },
  });
}

/** 组合所有 CodeMirror 扩展（runGutter 由 EditorPane 单独装入 compartment，便于状态刷新） */
export function httpExtensions(
  checkDefined: (name: string) => boolean,
) {
  return [
    httpLanguage,
    syntaxHighlighting(httpHighlightStyle),
    curlPasteSupport(),
    variableDecoration(checkDefined),
    blockDecoration(),
    activeBlockDecoration(),
  ];
}
