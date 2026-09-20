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
import {
  DEFAULT_FONT_SIZE,
  FONT_SIZES,
  buildEditorFontStack,
  buildFontScale,
  buildUiFontStack,
} from '../lib/fonts';
import type { UpdateSource } from '../lib/updates';

const STORAGE_KEY = 'http-client-pro:settings';

/** Editor 长行折行的默认值（默认开启） */
export const DEFAULT_EDITOR_LINE_WRAP = true;

/** 标签栏位置：头部（默认）/ 左侧 / 右侧 */
export type TabPosition = 'top' | 'left' | 'right';
/** 标签展示方式：单行（默认，横向滚动）/ 多行（自动换行） */
export type TabLayout = 'single' | 'multi';

export const TAB_POSITIONS: TabPosition[] = ['top', 'left', 'right'];
export const TAB_LAYOUTS: TabLayout[] = ['single', 'multi'];
export const DEFAULT_TAB_POSITION: TabPosition = 'top';
export const DEFAULT_TAB_LAYOUT: TabLayout = 'single';
/** 最大标签页数量默认值；超出时按打开时间淘汰已保存标签 */
export const DEFAULT_MAX_TABS = 5;
export const MAX_TABS_MIN = 1;
export const MAX_TABS_MAX = 50;

export interface SettingsSnapshot {
  themeId: string;
  editorSchemeId: string;
  uiFont: string;
  uiFontFallback: string;
  fontSize: number;
  /** Editor 长行折行显示 */
  editorLineWrap: boolean;
  /** 标签栏位置 */
  tabPosition: TabPosition;
  /** 标签展示方式（单行 / 多行） */
  tabLayout: TabLayout;
  /** 最大标签页数量 */
  maxTabs: number;
  keymapScheme: KeymapSchemeId;
  updateSource: UpdateSource;
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
  /** 全局字号（px），通过 --font-scale 缩放全站文字 */
  const fontSize = ref<number>(DEFAULT_FONT_SIZE);
  /** Editor 长行折行显示（关闭时长行横向滚动） */
  const editorLineWrap = ref<boolean>(DEFAULT_EDITOR_LINE_WRAP);
  /** 标签栏位置（头部 / 左侧 / 右侧） */
  const tabPosition = ref<TabPosition>(DEFAULT_TAB_POSITION);
  /** 标签展示方式（单行横向滚动 / 多行换行） */
  const tabLayout = ref<TabLayout>(DEFAULT_TAB_LAYOUT);
  /** 最大标签页数量，超出时淘汰最早打开的已保存标签 */
  const maxTabs = ref<number>(DEFAULT_MAX_TABS);
  /** 快捷键方案 */
  const keymapScheme = ref<KeymapSchemeId>(DEFAULT_KEYMAP_SCHEME);
  /** 更新下载源：github（官方）| cnb（国内镜像） */
  const updateSource = ref<UpdateSource>('github');

  const theme = computed<UiTheme>(() => getTheme(themeId.value));

  /** 编辑器实际使用的 color scheme（follow 时解析为主题自带 scheme） */
  const resolvedEditorScheme = computed<EditorScheme>(() => {
    if (editorSchemeId.value === EDITOR_SCHEME_FOLLOW) {
      return getEditorScheme(theme.value.editorScheme);
    }
    return getEditorScheme(editorSchemeId.value);
  });

  const uiFontFamily = computed(() => buildUiFontStack(uiFont.value, uiFontFallback.value));

  /** 编辑器字体栈（与界面字体同源；留空时使用内置等宽栈） */
  const editorFontFamily = computed(() => buildEditorFontStack(uiFont.value, uiFontFallback.value));

  /** 全站字号缩放系数（1 = 13px 基准） */
  const fontScale = computed(() => buildFontScale(fontSize.value));

  /** 将当前设置应用到 DOM（CSS 变量、字体、color-scheme） */
  function applyDom() {
    if (typeof document === 'undefined') return;
    applyThemeVars(theme.value.vars);
    const root = document.documentElement;
    root.style.setProperty('--font-ui', uiFontFamily.value);
    root.style.setProperty('--font-editor', editorFontFamily.value);
    root.style.setProperty('--font-scale', String(fontScale.value));
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
      if (typeof parsed.fontSize === 'number' && FONT_SIZES.includes(parsed.fontSize)) {
        fontSize.value = parsed.fontSize;
      }
      if (typeof parsed.editorLineWrap === 'boolean') {
        editorLineWrap.value = parsed.editorLineWrap;
      }
      if (typeof parsed.tabPosition === 'string' && TAB_POSITIONS.includes(parsed.tabPosition as TabPosition)) {
        tabPosition.value = parsed.tabPosition as TabPosition;
      }
      if (typeof parsed.tabLayout === 'string' && TAB_LAYOUTS.includes(parsed.tabLayout as TabLayout)) {
        tabLayout.value = parsed.tabLayout as TabLayout;
      }
      if (
        typeof parsed.maxTabs === 'number' && Number.isInteger(parsed.maxTabs) &&
        parsed.maxTabs >= MAX_TABS_MIN && parsed.maxTabs <= MAX_TABS_MAX
      ) {
        maxTabs.value = parsed.maxTabs;
      }
      if (parsed.keymapScheme && parsed.keymapScheme in KEYMAPS) {
        keymapScheme.value = parsed.keymapScheme;
      }
      if (parsed.updateSource === 'github' || parsed.updateSource === 'cnb') {
        updateSource.value = parsed.updateSource;
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
      fontSize: fontSize.value,
      editorLineWrap: editorLineWrap.value,
      tabPosition: tabPosition.value,
      tabLayout: tabLayout.value,
      maxTabs: maxTabs.value,
      keymapScheme: keymapScheme.value,
      updateSource: updateSource.value,
    };
  }

  function restore(s: SettingsSnapshot) {
    themeId.value = s.themeId;
    editorSchemeId.value = s.editorSchemeId;
    uiFont.value = s.uiFont;
    uiFontFallback.value = s.uiFontFallback;
    fontSize.value = s.fontSize;
    editorLineWrap.value = s.editorLineWrap;
    tabPosition.value = s.tabPosition;
    tabLayout.value = s.tabLayout;
    maxTabs.value = s.maxTabs;
    keymapScheme.value = s.keymapScheme;
    updateSource.value = s.updateSource;
  }

  /** 应用设置 = 持久化（DOM 已由监听器实时应用，便于预览） */
  function apply() {
    persist();
  }

  // 实时应用到 DOM（预览），不持久化；仅"应用"按钮触发持久化。
  watch([themeId, editorSchemeId, uiFont, uiFontFallback, fontSize], () => applyDom(), { flush: 'post' });

  // 标签页设置即时生效并立即持久化（不经"应用"按钮），
  // 避免关闭设置页时 restore 把预览中的布局改动悄悄回退。
  watch([tabPosition, tabLayout, maxTabs], () => persist(), { flush: 'sync' });

  return {
    themeId,
    editorSchemeId,
    uiFont,
    uiFontFallback,
    fontSize,
    editorLineWrap,
    tabPosition,
    tabLayout,
    maxTabs,
    keymapScheme,
    updateSource,
    theme,
    resolvedEditorScheme,
    uiFontFamily,
    editorFontFamily,
    fontScale,
    applyDom,
    load,
    persist,
    snapshot,
    restore,
    apply,
  };
});
