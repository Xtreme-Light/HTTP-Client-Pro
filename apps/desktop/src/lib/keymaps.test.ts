import { describe, it, expect } from 'vitest';
import {
  parseBinding,
  eventMatchesBinding,
  formatBinding,
  KEYMAPS,
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
