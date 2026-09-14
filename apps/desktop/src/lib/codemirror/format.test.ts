import { describe, it, expect } from 'vitest';
import { EditorView } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import {
  formatJson,
  isJsonBody,
  findBodyRegions,
  formatHttpEdits,
  formatHttpSource,
  formatDocumentCommand,
} from './format';

/** 把行数组拼成文档 */
const doc = (...lines: string[]) => lines.join('\n');

describe('formatJson', () => {
  it('pretty-prints a compact object', () => {
    expect(formatJson('{"a":1,"b":[1,2]}')).toBe(
      ['{', '  "a": 1,', '  "b": [', '    1,', '    2', '  ]', '}'].join('\n'),
    );
  });

  it('honors a custom indent width', () => {
    expect(formatJson('{"a":1}', 4)).toBe(['{', '    "a": 1', '}'].join('\n'));
  });

  it('returns null for invalid json', () => {
    expect(formatJson('{"a":}')).toBeNull();
    expect(formatJson('hello world')).toBeNull();
    expect(formatJson('')).toBeNull();
  });

  it('restores a bare variable used as a value', () => {
    expect(formatJson('{"id":{{uuid}},"n":1}')).toBe(
      ['{', '  "id": {{uuid}},', '  "n": 1', '}'].join('\n'),
    );
  });

  it('restores a variable embedded in a string value', () => {
    expect(formatJson('{"a":"prefix-{{token}}-suffix"}')).toBe(
      ['{', '  "a": "prefix-{{token}}-suffix"', '}'].join('\n'),
    );
  });

  it('keeps quotes when the whole string value is a variable', () => {
    expect(formatJson('{"a":"{{v}}","b":2}')).toBe(
      ['{', '  "a": "{{v}}",', '  "b": 2', '}'].join('\n'),
    );
  });

  it('supports whitespace inside variable braces', () => {
    expect(formatJson('{"id": {{ uuid }} }')).toBe(['{', '  "id": {{ uuid }}', '}'].join('\n'));
  });

  it('preserves a variable used as an object key', () => {
    expect(formatJson('{{{k}}: 1}')).toBe(['{', '  {{k}}: 1', '}'].join('\n'));
  });

  it('leaves non-variable double braces inside strings as-is', () => {
    expect(formatJson('{"a":"{{ }}"}')).toBe(['{', '  "a": "{{ }}"', '}'].join('\n'));
  });

  it('handles escaped quotes inside strings', () => {
    const input = '{"a":"x\\"{{v}}","b":{{n}}}';
    expect(formatJson(input)).toBe(
      ['{', '  "a": "x\\"{{v}}",', '  "b": {{n}}', '}'].join('\n'),
    );
  });

  it('does not treat braces in strings as variable starts', () => {
    expect(formatJson('{"a":"{{{b}}}"}')).toBe(['{', '  "a": "{{{b}}}"', '}'].join('\n'));
  });

  it('is idempotent', () => {
    const once = formatJson('{"a":[1,{"b":2}]}')!;
    expect(formatJson(once)).toBe(once);
  });
});

describe('isJsonBody', () => {
  it('detects by content type', () => {
    expect(isJsonBody('whatever', 'application/json')).toBe(true);
    expect(isJsonBody('whatever', 'application/vnd.api+json; charset=utf-8')).toBe(true);
    expect(isJsonBody('whatever', 'text/plain')).toBe(false);
  });

  it('detects by shape when content type is absent', () => {
    expect(isJsonBody('{"a":1}', null)).toBe(true);
    expect(isJsonBody('  [1,2]', null)).toBe(true);
    expect(isJsonBody('hello', null)).toBe(false);
    expect(isJsonBody('<note/>', 'application/xml')).toBe(false);
  });
});

describe('findBodyRegions', () => {
  it('finds the body after headers and captures content type', () => {
    const src = doc(
      'POST https://api.example.com/users',
      'Content-Type: application/json',
      '',
      '{"a":1}',
    );
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 3, endLine: 3, contentType: 'application/json' },
    ]);
  });

  it('finds a body without headers (blank line after request line)', () => {
    const src = doc('POST /a', '', '{"a":1}', '  ,{"b":2}');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 2, endLine: 3, contentType: null },
    ]);
  });

  it('accepts a bare URL request line', () => {
    const src = doc('https://api.example.com/a', '', '{"a":1}');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 2, endLine: 2, contentType: null },
    ]);
  });

  it('returns nothing for a request without body', () => {
    const src = doc('GET /a', 'Accept: application/json', '');
    expect(findBodyRegions(src.split('\n'))).toEqual([]);
  });

  it('returns nothing for comments and separators only', () => {
    const src = doc('# note', '### name', '', '// another');
    expect(findBodyRegions(src.split('\n'))).toEqual([]);
  });

  it('handles multiple blocks', () => {
    const src = doc(
      '### first',
      'POST /a',
      'Content-Type: application/json',
      '',
      '{"a":1}',
      '',
      '### second',
      'GET /b',
      '',
    );
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 4, endLine: 4, contentType: 'application/json' },
    ]);
  });

  it('excludes leading and trailing blank lines of the body', () => {
    const src = doc('POST /a', '', '', '{"a":1}', '', '');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 3, endLine: 3, contentType: null },
    ]);
  });

  it('keeps blank lines inside a multi-line body', () => {
    const src = doc('POST /a', '', 'line1', '', 'line2');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 2, endLine: 4, contentType: null },
    ]);
  });

  it('stops the body at trailer lines', () => {
    const src = doc(
      'POST /a',
      '',
      '{"a":1}',
      '',
      '> {% client.test("ok", function () {}); %}',
    );
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 2, endLine: 2, contentType: null },
    ]);
  });

  it('stops the body at response ref and redirect lines', () => {
    const src = doc('POST /a', '', '{"a":1}', '<> ./resp.json', '>> ./out.json');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 2, endLine: 2, contentType: null },
    ]);
  });

  it('does not treat xml body lines starting with < as trailers', () => {
    const src = doc(
      'POST /x',
      'Content-Type: application/xml',
      '',
      '<note>',
      '  <to>a</to>',
      '</note>',
    );
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 3, endLine: 5, contentType: 'application/xml' },
    ]);
  });

  it('keeps content type value verbatim including parameters', () => {
    const src = doc('POST /a', 'content-type: application/JSON; charset=utf-8', '', '{}');
    expect(findBodyRegions(src.split('\n'))).toEqual([
      { startLine: 3, endLine: 3, contentType: 'application/JSON; charset=utf-8' },
    ]);
  });

  it('does not start a body before any request line', () => {
    const src = doc('# hello', '', 'not a body');
    expect(findBodyRegions(src.split('\n'))).toEqual([]);
  });
});

describe('formatHttpEdits', () => {
  it('returns character offsets of the body region', () => {
    const src = 'POST /a\n\n{"a":1}\n';
    const edits = formatHttpEdits(src);
    expect(edits).toHaveLength(1);
    expect(edits[0].from).toBe(9);
    expect(edits[0].to).toBe(16);
    expect(src.slice(edits[0].from, edits[0].to)).toBe('{"a":1}');
    expect(edits[0].insert).toBe(['{', '  "a": 1', '}'].join('\n'));
  });

  it('skips non-json bodies', () => {
    const src = doc('POST /a', 'Content-Type: text/plain', '', 'hello world', '');
    expect(formatHttpEdits(src)).toEqual([]);
  });

  it('skips bodies that are already formatted', () => {
    const src = doc('POST /a', '', ['{', '  "a": 1', '}'].join('\n'));
    expect(formatHttpEdits(src)).toEqual([]);
  });

  it('skips an invalid json body', () => {
    const src = doc('POST /a', 'Content-Type: application/json', '', '{"a":}', '');
    expect(formatHttpEdits(src)).toEqual([]);
  });
});

describe('formatHttpSource', () => {
  it('formats every json body and leaves other bodies untouched', () => {
    const src = doc(
      '### create',
      'POST https://x/a',
      'Content-Type: application/json',
      '',
      '{"id":{{uuid}},"tags":["a","b"]}',
      '',
      '### text',
      'POST https://x/b',
      'Content-Type: text/plain',
      '',
      'hello world',
      '',
    );
    const { text, formatted } = formatHttpSource(src);
    expect(formatted).toBe(1);
    expect(text).toBe(
      doc(
        '### create',
        'POST https://x/a',
        'Content-Type: application/json',
        '',
        '{',
        '  "id": {{uuid}},',
        '  "tags": [',
        '    "a",',
        '    "b"',
        '  ]',
        '}',
        '',
        '### text',
        'POST https://x/b',
        'Content-Type: text/plain',
        '',
        'hello world',
        '',
      ),
    );
  });

  it('is idempotent', () => {
    const src = doc('POST /a', '', '{"a":{"b":[1,2]}}', '');
    const first = formatHttpSource(src);
    const second = formatHttpSource(first.text);
    expect(first.formatted).toBe(1);
    expect(second.formatted).toBe(0);
    expect(second.text).toBe(first.text);
  });

  it('preserves CRLF line endings', () => {
    const src = 'POST /a\r\n\r\n{"a":1}\r\n';
    const { text, formatted } = formatHttpSource(src);
    expect(formatted).toBe(1);
    expect(text).toBe('POST /a\r\n\r\n{\r\n  "a": 1\r\n}\r\n');
    expect(text).not.toContain('\r\n\r\n\n');
  });

  it('leaves the document unchanged when nothing is formattable', () => {
    const src = doc('GET /a', '', '### nothing', '');
    const { text, formatted } = formatHttpSource(src);
    expect(formatted).toBe(0);
    expect(text).toBe(src);
  });
});

describe('formatDocumentCommand', () => {
  function makeView(source: string) {
    return new EditorView({ state: EditorState.create({ doc: source }) });
  }

  it('formats the document and returns true', () => {
    const view = makeView(doc('POST /a', '', '{"a":1}'));
    expect(formatDocumentCommand(view)).toBe(true);
    expect(view.state.sliceDoc()).toBe(doc('POST /a', '', ['{', '  "a": 1', '}'].join('\n')));
    view.destroy();
  });

  it('returns false when there is nothing to format', () => {
    const view = makeView(doc('GET /a'));
    expect(formatDocumentCommand(view)).toBe(false);
    expect(view.state.sliceDoc()).toBe(doc('GET /a'));
    view.destroy();
  });
});
