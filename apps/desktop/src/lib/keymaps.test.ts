import { describe, it, expect } from 'vitest';
import {
  parseBinding,
  eventMatchesBinding,
  formatBinding,
  KEYMAPS,
  KEYMAP_SCHEME_LIST,
  SHORTCUT_ACTIONS,
} from './keymaps';

function ev(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', init);
}

describe('parseBinding', () => {
  it('parses modifiers and key', () => {
    expect(parseBinding('Mod-Shift-s')).toEqual({
      mod: true,
      shift: true,
      alt: false,
      ctrl: false,
      key: 's',
    });
  });

  it('parses key that is a dash (Mod--)', () => {
    expect(parseBinding('Mod--')).toEqual({
      mod: true,
      shift: false,
      alt: false,
      ctrl: false,
      key: '-',
    });
  });

  it('parses named keys (Enter)', () => {
    expect(parseBinding('Mod-Enter')?.key).toBe('Enter');
  });

  it('parses Mod-Shift-Alt-s', () => {
    expect(parseBinding('Mod-Shift-Alt-s')).toEqual({
      mod: true,
      shift: true,
      alt: true,
      ctrl: false,
      key: 's',
    });
  });

  it('returns null for empty/invalid binding', () => {
    expect(parseBinding('')).toBeNull();
    expect(parseBinding('Mod-')).toBeNull();
  });
});

describe('eventMatchesBinding', () => {
  it('matches Ctrl+S against Mod-s', () => {
    expect(eventMatchesBinding(ev({ key: 's', ctrlKey: true }), 'Mod-s')).toBe(true);
    expect(eventMatchesBinding(ev({ key: 'S', ctrlKey: true }), 'Mod-s')).toBe(true);
  });

  it('requires exact modifier set', () => {
    // Ctrl+Shift+S 不匹配 Mod-s
    expect(eventMatchesBinding(ev({ key: 's', ctrlKey: true, shiftKey: true }), 'Mod-s')).toBe(false);
    // Ctrl+Shift+S 匹配 Mod-Shift-s
    expect(eventMatchesBinding(ev({ key: 's', ctrlKey: true, shiftKey: true }), 'Mod-Shift-s')).toBe(true);
    // 无修饰键不匹配
    expect(eventMatchesBinding(ev({ key: 's' }), 'Mod-s')).toBe(false);
  });

  it('matches special keys', () => {
    expect(eventMatchesBinding(ev({ key: 'Enter', ctrlKey: true }), 'Mod-Enter')).toBe(true);
    expect(eventMatchesBinding(ev({ key: '-', ctrlKey: true }), 'Mod--')).toBe(true);
    expect(eventMatchesBinding(ev({ key: '=', ctrlKey: true }), 'Mod-=')).toBe(true);
    expect(eventMatchesBinding(ev({ key: '0', ctrlKey: true }), 'Mod-0')).toBe(true);
  });

  it('matches Alt-only bindings (Alt-1)', () => {
    expect(eventMatchesBinding(ev({ key: '1', altKey: true }), 'Alt-1')).toBe(true);
    expect(eventMatchesBinding(ev({ key: '1' }), 'Alt-1')).toBe(false);
  });

  it('ignores IME composition events', () => {
    expect(eventMatchesBinding(ev({ key: 's', ctrlKey: true, isComposing: true }), 'Mod-s')).toBe(false);
  });

  it('metaKey counts as Mod', () => {
    expect(eventMatchesBinding(ev({ key: 's', metaKey: true }), 'Mod-s')).toBe(true);
  });

  it('all builtin scheme bindings are parseable', () => {
    for (const scheme of Object.values(KEYMAPS)) {
      for (const [action, binding] of Object.entries(scheme.bindings)) {
        expect(parseBinding(binding), `${scheme.id}.${action} = ${binding}`).not.toBeNull();
        expect(formatBinding(binding).length, `${scheme.id}.${action} display`).toBeGreaterThan(0);
      }
    }
  });
});

describe('formatDocument shortcut', () => {
  it('is listed in the settings shortcut table under 编辑', () => {
    const def = SHORTCUT_ACTIONS.find((a) => a.id === 'formatDocument');
    expect(def).toBeDefined();
    expect(def?.group).toBe('编辑');
  });

  it('has a binding in every keymap scheme', () => {
    for (const scheme of KEYMAP_SCHEME_LIST) {
      expect(scheme.bindings.formatDocument, scheme.id).toBeTruthy();
      expect(formatBinding(scheme.bindings.formatDocument).length).toBeGreaterThan(0);
    }
  });

  it('matches the expected key combos', () => {
    expect(eventMatchesBinding(ev({ key: 'l', ctrlKey: true, altKey: true }), KEYMAPS.windows.bindings.formatDocument)).toBe(true);
    expect(eventMatchesBinding(ev({ key: 'F', shiftKey: true, altKey: true }), KEYMAPS.vscode.bindings.formatDocument)).toBe(true);
    expect(eventMatchesBinding(ev({ key: 'l', ctrlKey: true, altKey: true }), KEYMAPS.jetbrains.bindings.formatDocument)).toBe(true);
  });

  it('does not collide with other bindings in the same scheme', () => {
    for (const scheme of KEYMAP_SCHEME_LIST) {
      const seen = new Map<string, string>();
      for (const [action, binding] of Object.entries(scheme.bindings)) {
        const key = binding.toLowerCase();
        expect(seen.get(key), `${scheme.id}: ${binding} 冲突于 ${seen.get(key)} / ${action}`).toBeUndefined();
        seen.set(key, action);
      }
    }
  });
});
