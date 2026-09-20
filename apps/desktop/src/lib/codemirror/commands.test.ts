import { describe, it, expect } from 'vitest';
import { EditorSelection, EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { computeGotoNextLine, gotoNextLineCommand } from './commands';

/** 把行数组拼成文档 */
const doc = (...lines: string[]) => lines.join('\n');

function state(source: string, cursor: number) {
  return EditorState.create({ doc: source, selection: EditorSelection.cursor(cursor) });
}

function makeView(source: string, cursor: number) {
  return new EditorView({ state: state(source, cursor) });
}

describe('computeGotoNextLine', () => {
  it('moves to the next line keeping the column, without editing the doc', () => {
    // "ABCDE\nFGHIJ"：光标在 B、C 之间（位置 2）→ 下一行同列（位置 8，即 H 前）
    const { changes, selection } = computeGotoNextLine(state(doc('ABCDE', 'FGHIJ'), 2));
    expect(changes).toEqual([]);
    expect(selection[0].head).toBe(8);
  });

  it('clamps to the line end when the next line is shorter', () => {
    // "ABCDE\nFG"：光标在 E 前（位置 4）→ 下一行只有 2 个字符，落到行尾（位置 8）
    const { changes, selection } = computeGotoNextLine(state(doc('ABCDE', 'FG'), 4));
    expect(changes).toEqual([]);
    expect(selection[0].head).toBe(8);
  });

  it('appends a newline when already on the last line', () => {
    const { changes, selection } = computeGotoNextLine(state('ABCDE', 2));
    expect(changes).toEqual([{ from: 5, insert: '\n' }]);
    expect(selection[0].head).toBe(6);
  });

  it('appends a newline at the document end for a multi-line doc', () => {
    const { changes, selection } = computeGotoNextLine(state(doc('A', 'B'), 3));
    expect(changes).toEqual([{ from: 3, insert: '\n' }]);
    expect(selection[0].head).toBe(4);
  });

  it('creates a new line when the last line is empty', () => {
    const { changes, selection } = computeGotoNextLine(state(doc('A', ''), 2));
    expect(changes).toEqual([{ from: 2, insert: '\n' }]);
    expect(selection[0].head).toBe(3);
  });
});

describe('gotoNextLineCommand', () => {
  it('jumps to the next line and leaves the doc untouched', () => {
    const view = makeView(doc('ABCDE', 'FGHIJ'), 2);
    expect(gotoNextLineCommand(view)).toBe(true);
    expect(view.state.sliceDoc()).toBe(doc('ABCDE', 'FGHIJ'));
    expect(view.state.selection.main.head).toBe(8);
    view.destroy();
  });

  it('creates the next line when there is none', () => {
    const view = makeView('ABCDE', 2);
    expect(gotoNextLineCommand(view)).toBe(true);
    expect(view.state.sliceDoc()).toBe('ABCDE\n');
    expect(view.state.selection.main.head).toBe(6);
    view.destroy();
  });
});
