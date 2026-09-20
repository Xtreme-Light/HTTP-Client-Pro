/**
 * 编辑器自定义命令（按键触发，签名统一为 `(view: EditorView) => boolean`）。
 * 在 `EditorPane.vue::buildAppKeymap()` 中按快捷键方案注册。
 */
import { EditorSelection } from '@codemirror/state';
import type { ChangeSpec, EditorState, SelectionRange } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

export interface GotoNextLineUpdate {
  changes: ChangeSpec[];
  selection: SelectionRange[];
}

/**
 * 计算「跳到下一行」的文档变更与光标落点（纯函数，便于单测）。
 *
 * - 光标不在最后一行 → 移到下一行，保留当前列号（下一行更短则贴到行尾）；
 * - 光标在最后一行 → 在文档末尾追加一个换行，光标落到新行行首。
 */
export function computeGotoNextLine(state: EditorState): GotoNextLineUpdate {
  const lastLineNo = state.doc.lines;
  const docEnd = state.doc.length;
  const changes: ChangeSpec[] = [];

  // 只要有一个光标停在最后一行，就在文档末尾补一个换行；
  // 追加位置在所有已存在行之后，其余光标的行位置无需映射。
  const needNewLine = state.selection.ranges.some(
    (r) => state.doc.lineAt(r.head).number === lastLineNo,
  );
  if (needNewLine) changes.push({ from: docEnd, insert: '\n' });

  const selection = state.selection.ranges.map((r) => {
    const line = state.doc.lineAt(r.head);
    if (line.number < lastLineNo) {
      const next = state.doc.line(line.number + 1);
      const col = Math.min(r.head - line.from, next.length);
      return EditorSelection.cursor(next.from + col);
    }
    return EditorSelection.cursor(docEnd + 1);
  });

  return { changes, selection };
}

/** 跳到下一行；已在最后一行时在文档末尾新建一行 */
export function gotoNextLineCommand(view: EditorView): boolean {
  const { state } = view;
  const { changes, selection } = computeGotoNextLine(state);
  view.dispatch({
    changes,
    selection: EditorSelection.create(selection, state.selection.mainIndex),
    scrollIntoView: true,
    // 纯移动不入撤销栈；追加换行按输入处理
    userEvent: changes.length > 0 ? 'input' : 'select',
  });
  return true;
}
