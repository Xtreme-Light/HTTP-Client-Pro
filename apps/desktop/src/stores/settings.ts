import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import {
  DEFAULT_THEME_ID,
  EDITOR_SCHEME_FOLLOW,
  EDITOR_SCHEMES,
  UI_THEMES,
  applyThemeVars,
  getEditorScheme,
  getTheme,
  type EditorScheme,
  type UiTheme,
} from '../lib/themes';
import { DEFAULT_KEYMAP_SCHEME, KEYMAPS, type KeymapSchemeId } from '../lib/keymaps';
import { buildEditorFontStack, buildUiFontStack } from '../lib/fonts';

const STORAGE_KEY = 'http-client-pro:settings';

export interface SettingsSnapshot {
  themeId: string;
  editorSchemeId: string;
  uiFont: string;
  uiFontFallback: string;
  keymapScheme: KeymapSchemeId;
}

export const useSettingsStore = defineStore('settings', () => {
  /** UI 主题 id */
  const themeId = ref<string>(DEFAULT_THEME_ID);
  /** 编辑器 color scheme id；'follow' 表示跟随 UI 主题 */
  const editorSchemeId = ref<string>(EDITOR_SCHEME_FOLLOW);
  /** 界面主字体（空 = 跟随系统默认栈） */
  const uiFont = ref('');
  /** 界面字体 fallback（逗号分隔） */
  const uiFontFallback = ref('');
  /** 快捷键方案 */
  const keymapScheme = ref<KeymapSchemeId>(DEFAULT_KEYMAP_SCHEME);

  const theme = computed<UiTheme>(() => getTheme(themeId.value));

  /** 编辑器实际使用的 color scheme（follow 时解析为主题自带 scheme） */
  const resolvedEditorScheme = computed<EditorScheme>(() => {
    if (editorSchemeId.value === EDITOR_SCHEME_FOLLOW) {
      return getEditorScheme(theme.value.editorScheme);
    }
    return getEditorScheme(editorSchemeId.value);
  });

  const uiFontFamily = computed(() => buildUiFontStack(uiFont.value, uiFontFallback.value));

  /** 编辑器 / Console 字体栈（与界面字体同源；留空时使用内置等宽栈） */
  const editorFontFamily = computed(() => buildEditorFontStack(uiFont.value, uiFontFallback.value));

  /** 将当前设置应用到 DOM（CSS 变量、字体、color-scheme） */
  function applyDom() {
    if (typeof document === 'undefined') return;
    applyThemeVars(theme.value.vars);
    const root = document.documentElement;
    root.style.setProperty('--font-ui', uiFontFamily.value);
    root.style.setProperty('--font-editor', editorFontFamily.value);
    root.style.setProperty('color-scheme', theme.value.base);
  }

  /** 从 localStorage 加载（校验合法性，非法值保持默认） */
  function load() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw) as Partial<SettingsSnapshot>;
      if (!parsed || typeof parsed !== 'object') return;
      if (typeof parsed.themeId === 'string' && UI_THEMES.some((t) => t.id === parsed.themeId)) {
        themeId.value = parsed.themeId;
      }
      if (
        typeof parsed.editorSchemeId === 'string' &&
        (parsed.editorSchemeId === EDITOR_SCHEME_FOLLOW ||
          EDITOR_SCHEMES.some((s) => s.id === parsed.editorSchemeId))
      ) {
        editorSchemeId.value = parsed.editorSchemeId;
      }
      if (typeof parsed.uiFont === 'string') uiFont.value = parsed.uiFont;
      if (typeof parsed.uiFontFallback === 'string') uiFontFallback.value = parsed.uiFontFallback;
      if (parsed.keymapScheme && parsed.keymapScheme in KEYMAPS) {
        keymapScheme.value = parsed.keymapScheme;
      }
    } catch {
      /* ignore */
    }
  }

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot()));
    } catch {
      /* ignore */
    }
  }

  function snapshot(): SettingsSnapshot {
    return {
      themeId: themeId.value,
      editorSchemeId: editorSchemeId.value,
      uiFont: uiFont.value,
      uiFontFallback: uiFontFallback.value,
      keymapScheme: keymapScheme.value,
    };
  }

  function restore(s: SettingsSnapshot) {
    themeId.value = s.themeId;
    editorSchemeId.value = s.editorSchemeId;
    uiFont.value = s.uiFont;
    uiFontFallback.value = s.uiFontFallback;
    keymapScheme.value = s.keymapScheme;
  }

  /** 应用设置 = 持久化（DOM 已由监听器实时应用，便于预览） */
  function apply() {
    persist();
  }

  // 实时应用到 DOM（预览），不持久化；仅"应用"按钮触发持久化。
  watch([themeId, editorSchemeId, uiFont, uiFontFallback], () => applyDom(), { flush: 'post' });

  return {
    themeId,
    editorSchemeId,
    uiFont,
    uiFontFallback,
    keymapScheme,
    theme,
    resolvedEditorScheme,
    uiFontFamily,
    editorFontFamily,
    applyDom,
    load,
    persist,
    snapshot,
    restore,
    apply,
  };
});
