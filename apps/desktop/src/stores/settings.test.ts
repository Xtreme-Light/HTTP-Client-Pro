import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { DEFAULT_FONT_SIZE, FONT_SIZES, buildFontScale } from '../lib/fonts';
import {
  DEFAULT_EDITOR_LINE_WRAP,
  DEFAULT_MAX_TABS,
  DEFAULT_TAB_LAYOUT,
  DEFAULT_TAB_POSITION,
  useSettingsStore,
} from './settings';

const STORAGE_KEY = 'http-client-pro:settings';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  document.documentElement.removeAttribute('style');
});

describe('buildFontScale', () => {
  it('returns 1 for the base size', () => {
    expect(DEFAULT_FONT_SIZE).toBe(13);
    expect(buildFontScale(13)).toBe(1);
  });

  it('scales proportionally to the base size', () => {
    expect(buildFontScale(26)).toBe(2);
    expect(buildFontScale(16)).toBeCloseTo(1.231, 3);
  });

  it('falls back to 1 for invalid sizes', () => {
    expect(buildFontScale(0)).toBe(1);
    expect(buildFontScale(-1)).toBe(1);
    expect(buildFontScale(Number.NaN)).toBe(1);
  });
});

describe('settings fontSize', () => {
  it('defaults to the base size and writes --font-scale to the DOM', () => {
    const settings = useSettingsStore();
    expect(settings.fontSize).toBe(DEFAULT_FONT_SIZE);
    settings.applyDom();
    expect(document.documentElement.style.getPropertyValue('--font-scale')).toBe('1');
  });

  it('writes a scaled --font-scale when the size changes', () => {
    const settings = useSettingsStore();
    settings.fontSize = 18;
    settings.applyDom();
    expect(document.documentElement.style.getPropertyValue('--font-scale')).toBe(
      String(buildFontScale(18)),
    );
  });

  it('round-trips through the snapshot', () => {
    const settings = useSettingsStore();
    settings.fontSize = 15;
    settings.apply();
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}').fontSize).toBe(15);

    // 换一个 pinia 实例模拟重启后从 localStorage 恢复
    setActivePinia(createPinia());
    const reloaded = useSettingsStore();
    reloaded.load();
    expect(reloaded.fontSize).toBe(15);
    expect(reloaded.snapshot().fontSize).toBe(15);
  });

  it('ignores sizes outside the allowed list', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ fontSize: 99 }));
    const settings = useSettingsStore();
    settings.load();
    expect(settings.fontSize).toBe(DEFAULT_FONT_SIZE);
  });

  it('ignores non-numeric sizes', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ fontSize: '14' }));
    const settings = useSettingsStore();
    settings.load();
    expect(settings.fontSize).toBe(DEFAULT_FONT_SIZE);
  });

  it('restores the size when discarding changes', () => {
    const settings = useSettingsStore();
    const before = settings.snapshot();
    settings.fontSize = FONT_SIZES[FONT_SIZES.length - 1];
    settings.restore(before);
    expect(settings.fontSize).toBe(DEFAULT_FONT_SIZE);
  });
});

describe('settings editorLineWrap', () => {
  it('defaults to enabled', () => {
    const settings = useSettingsStore();
    expect(DEFAULT_EDITOR_LINE_WRAP).toBe(true);
    expect(settings.editorLineWrap).toBe(true);
  });

  it('round-trips through the snapshot', () => {
    const settings = useSettingsStore();
    settings.editorLineWrap = false;
    settings.apply();
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}').editorLineWrap).toBe(false);

    // 换一个 pinia 实例模拟重启后从 localStorage 恢复
    setActivePinia(createPinia());
    const reloaded = useSettingsStore();
    reloaded.load();
    expect(reloaded.editorLineWrap).toBe(false);
  });

  it('ignores non-boolean values', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ editorLineWrap: 'off' }));
    const settings = useSettingsStore();
    settings.load();
    expect(settings.editorLineWrap).toBe(DEFAULT_EDITOR_LINE_WRAP);
  });

  it('restores the value when discarding changes', () => {
    const settings = useSettingsStore();
    const before = settings.snapshot();
    settings.editorLineWrap = false;
    settings.restore(before);
    expect(settings.editorLineWrap).toBe(DEFAULT_EDITOR_LINE_WRAP);
  });
});

describe('settings tab options', () => {
  it('defaults to top / single / 5', () => {
    const settings = useSettingsStore();
    expect(DEFAULT_TAB_POSITION).toBe('top');
    expect(DEFAULT_TAB_LAYOUT).toBe('single');
    expect(DEFAULT_MAX_TABS).toBe(5);
    expect(settings.tabPosition).toBe('top');
    expect(settings.tabLayout).toBe('single');
    expect(settings.maxTabs).toBe(5);
  });

  it('persists immediately on change (no apply needed)', () => {
    const settings = useSettingsStore();
    settings.tabPosition = 'left';
    settings.tabLayout = 'multi';
    settings.maxTabs = 8;
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
    expect(stored.tabPosition).toBe('left');
    expect(stored.tabLayout).toBe('multi');
    expect(stored.maxTabs).toBe(8);
  });

  it('round-trips through load after restart', () => {
    const settings = useSettingsStore();
    settings.tabPosition = 'right';
    settings.maxTabs = 12;

    setActivePinia(createPinia());
    const reloaded = useSettingsStore();
    reloaded.load();
    expect(reloaded.tabPosition).toBe('right');
    expect(reloaded.tabLayout).toBe('single');
    expect(reloaded.maxTabs).toBe(12);
  });

  it('ignores invalid position / layout', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ tabPosition: 'bottom', tabLayout: 'grid' }));
    const settings = useSettingsStore();
    settings.load();
    expect(settings.tabPosition).toBe('top');
    expect(settings.tabLayout).toBe('single');
  });

  it('ignores maxTabs out of range or non-integer', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ maxTabs: 0 }));
    let settings = useSettingsStore();
    settings.load();
    expect(settings.maxTabs).toBe(DEFAULT_MAX_TABS);

    setActivePinia(createPinia());
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ maxTabs: 999 }));
    settings = useSettingsStore();
    settings.load();
    expect(settings.maxTabs).toBe(DEFAULT_MAX_TABS);

    setActivePinia(createPinia());
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ maxTabs: 3.5 }));
    settings = useSettingsStore();
    settings.load();
    expect(settings.maxTabs).toBe(DEFAULT_MAX_TABS);
  });
});
