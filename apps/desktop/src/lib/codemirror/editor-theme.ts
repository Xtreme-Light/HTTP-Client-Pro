/**
 * 根据编辑器 Color Scheme 生成 CodeMirror 主题扩展。
 * 结构规则固定，颜色全部来自 EditorScheme，支持运行时重配置。
 */
import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import type { EditorScheme } from '../themes';

/**
 * 编辑器字体 — 读取全局 `--font-editor` CSS 变量。
 * 该变量由设置页的字体选项驱动（见 stores/settings.ts applyDom），
 * 默认等宽栈定义在 styles/global.css 与 lib/fonts.ts。
 */
const EDITOR_FONT_FAMILY = 'var(--font-editor)';

/** 统一行高 — 行号 gutter 与编辑区内容共用，保证每行等高且左右垂直居中对齐 */
export const EDITOR_LINE_HEIGHT = '1.6';

export function buildEditorTheme(scheme: EditorScheme): Extension {
  const s = scheme.syntax;
  return EditorView.theme({
    '&': { height: '100%', fontSize: '14px' },
    '.cm-scroller': {
      fontFamily: EDITOR_FONT_FAMILY,
      overflow: 'auto',
      backgroundColor: scheme.background,
    },
    '.cm-content': {
      backgroundColor: scheme.background,
      color: scheme.foreground,
      caretColor: scheme.caret,
      lineHeight: EDITOR_LINE_HEIGHT,
    },
    '.cm-cursor, .cm-dropCursor': {
      borderLeftColor: scheme.caret,
    },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection':
      { backgroundColor: scheme.selection },
    // 行号 gutter 样式 — 与编辑区使用相同字体和行高确保对齐
    '.cm-gutters': {
      borderRight: `1px solid ${scheme.gutterBorder}`,
      backgroundColor: scheme.gutterBg,
      color: scheme.gutterFg,
      display: 'flex',
      fontFamily: EDITOR_FONT_FAMILY,
    },
    '.cm-lineNumbers .cm-gutterElement': {
      padding: '0 8px',
      fontSize: '14px',
      minWidth: '3em',
      // 以行为盒模型中线为基准垂直居中，与右侧内容对齐
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'flex-end',
      lineHeight: EDITOR_LINE_HEIGHT,
    },
    '.cm-line': {
      padding: '0 4px',
      boxSizing: 'border-box',
    },
    // Run 按钮 gutter — 独立列，不占据编辑区域
    '.cm-run-gutter': {
      width: '28px',
      minWidth: '28px',
      borderRight: `1px solid ${scheme.gutterBorder}`,
    },
    '.cm-run-gutter .cm-gutterElement': {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: '0',
    },
    '.cm-separator': { color: s.separator },
    '.cm-separator-name': { color: s.separatorName, fontWeight: 'bold' },
    '.cm-separator-comment': { color: s.separatorComment },
    '.cm-comment': { color: s.comment, fontStyle: 'italic' },
    '.cm-method': { color: s.method, fontWeight: 'bold' },
    '.cm-url': { color: s.url },
    '.cm-header-name': { color: s.headerName },
    '.cm-header-value': { color: s.headerValue },
    '.cm-var': { color: s.variable },
    '.cm-var-defined': {
      color: s.varDefined,
      backgroundColor: s.varDefinedBg,
      borderRadius: '3px',
    },
    '.cm-var-undefined': {
      color: s.varUndefined,
      backgroundColor: s.varUndefinedBg,
      borderRadius: '3px',
      textDecoration: 'underline wavy',
    },
    '.cm-body': { color: s.body },
    // 块卡片化样式 — 分隔线用 inset shadow 绘制，不改变行盒高度，保证每行等高
    '.cm-block-start': {
      boxShadow: `inset 0 1px 0 0 ${s.blockBorder}`,
    },
    '.cm-request-line': {
      backgroundColor: s.requestLineBg,
      fontWeight: '500',
    },
    // 当前激活块 — 左侧竖线（JetBrains 风格）
    '.cm-active-block': {
      borderLeft: `2px solid ${scheme.accent}`,
      paddingLeft: '4px',
      marginLeft: '-4px',
    },
    // Run 按钮 gutter 内的按钮样式
    '.cm-run-btn': {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '20px',
      height: '20px',
      color: scheme.accent,
      cursor: 'pointer',
      fontSize: '11px',
      userSelect: 'none',
      borderRadius: '4px',
      transition: 'background 0.15s',
      position: 'relative',
    },
    '.cm-run-btn:hover': {
      background: scheme.accentSoft,
    },
    // 执行状态徽标 — 位于 ▶ 按钮右下角：执行中转圈动画 / 完成绿色√ / 失败红色✕
    '.cm-run-badge': {
      position: 'absolute',
      right: '-3px',
      bottom: '-3px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '11px',
      height: '11px',
      borderRadius: '50%',
      backgroundColor: scheme.gutterBg,
      pointerEvents: 'none',
    },
    '.cm-run-badge.running': {
      width: '10px',
      height: '10px',
      border: '1.5px solid var(--accent-soft)',
      borderTopColor: scheme.accent,
      animation: 'cm-run-spin 0.8s linear infinite',
    },
    '.cm-run-badge.done': {
      color: 'var(--success-hover)',
      fontSize: '9px',
      fontWeight: 'bold',
      lineHeight: '1',
    },
    '.cm-run-badge.error': {
      color: 'var(--danger)',
      fontSize: '8px',
      fontWeight: 'bold',
      lineHeight: '1',
    },
    // ---- 自动补全下拉框（见 lib/codemirror/completions.ts）----
    '.cm-tooltip.cm-tooltip-autocomplete': {
      backgroundColor: scheme.gutterBg,
      border: `1px solid ${scheme.gutterBorder}`,
      borderRadius: '4px',
      boxShadow: `0 6px 20px var(--shadow)`,
      overflow: 'hidden',
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul': {
      fontFamily: EDITOR_FONT_FAMILY,
      fontSize: '13px',
      maxHeight: '16em',
      maxWidth: 'min(560px, 90vw)',
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
      display: 'flex',
      alignItems: 'baseline',
      gap: '10px',
      padding: '2px 8px',
      color: scheme.foreground,
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: scheme.selection,
      color: scheme.foreground,
    },
    '.cm-completionLabel': {
      flex: '0 1 auto',
      whiteSpace: 'pre',
    },
    '.cm-completionMatchedText': {
      color: scheme.accent,
      textDecoration: 'none',
      fontWeight: '600',
    },
    '.cm-completionDetail': {
      flex: '1 1 auto',
      marginLeft: 'auto',
      fontStyle: 'italic',
      opacity: '0.7',
      color: s.comment,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
    },
    // 按候选类型着色，与编辑器语法高亮保持一致
    '.cm-http-completion-keyword .cm-completionLabel': { color: s.method, fontWeight: 'bold' },
    '.cm-http-completion-url .cm-completionLabel': { color: s.url },
    '.cm-http-completion-property .cm-completionLabel': { color: s.headerName },
    '.cm-http-completion-variable .cm-completionLabel': { color: s.variable },
  });
}
