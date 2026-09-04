/**
 * 主题系统定义 — UI 主题（CSS 变量）+ 编辑器 Color Scheme。
 *
 * UI 主题通过 CSS 变量驱动整个界面；编辑器 Color Scheme 驱动 CodeMirror 主题。
 * 编辑器 scheme 默认跟随 UI 主题（'follow'），也可在设置中独立指定。
 */

/** UI 主题 CSS 变量集合（键名不含 `--` 前缀） */
export interface UiThemeVars {
  /* 背景层级 */
  'bg-base': string;         // 主内容区（编辑区、面板内容）
  'bg-panel': string;        // 次级面板（侧边栏、标签栏、区块头）
  'bg-chrome': string;       // 应用外框（标题栏、下拉菜单、弹窗）
  'bg-hover': string;        // 悬停
  'bg-active': string;       // 激活项（菜单选中）
  'bg-input': string;        // 输入框/按钮基底
  'bg-input-deep': string;   // 深层输入（代码框、正文编辑）
  'bg-button': string;
  'bg-button-hover': string;

  /* 边框 */
  'border': string;          // 细分隔线
  'border-strong': string;   // 输入框/按钮边框
  'border-block': string;    // 代码块/卡片边框

  /* 前景文字 */
  'fg': string;              // 主文字
  'fg-secondary': string;    // 次要文字
  'fg-muted': string;        // 弱化文字
  'fg-strong': string;       // 强调文字（激活、悬停高亮）

  /* 强调与焦点 */
  'accent': string;          // 品牌强调色（激活指示、运行按钮）
  'accent-soft': string;     // 强调色浅底
  'focus': string;           // 焦点/链接蓝
  'focus-bg': string;        // 选中条目背景
  'focus-ring': string;      // 输入框聚焦光晕

  /* 语义色 */
  'success': string;
  'success-hover': string;
  'success-fg': string;
  'danger': string;
  'danger-strong': string;
  'danger-bg': string;
  'danger-border': string;
  'warning': string;

  /* 滚动条与分割条 */
  'scrollbar': string;
  'scrollbar-hover': string;
  'splitter': string;
  'splitter-hover': string;
  'shadow': string;
  'overlay': string;

  /* 状态徽标（method / status chips） */
  'chip-safe-bg': string;
  'chip-safe-fg': string;
  'chip-write-bg': string;
  'chip-write-fg': string;
  'chip-danger-bg': string;
  'chip-danger-fg': string;
  'chip-warn-bg': string;
  'chip-warn-fg': string;
  'chip-neutral-bg': string;
  'chip-neutral-fg': string;

  /* UI 中复用的语法色（块名、JSON key、字符串值等） */
  'syn-keyword': string;
  'syn-url': string;
  'syn-string': string;
  'syn-comment': string;
  'syn-number': string;
  'syn-bool': string;
  'syn-tag': string;
  'syn-attr': string;
}

export interface UiTheme {
  id: string;
  name: string;
  /** 明暗基底 — 影响 color-scheme 与表单控件渲染 */
  base: 'light' | 'dark';
  /** true = 内置主题；false = 开源主题 */
  builtin: boolean;
  /** 来源说明（开源协议等） */
  source: string;
  vars: UiThemeVars;
  /** 该主题默认的编辑器 Color Scheme id */
  editorScheme: string;
}

/** 编辑器语法配色 */
export interface EditorSchemeSyntax {
  separator: string;
  separatorName: string;
  separatorComment: string;
  comment: string;
  method: string;
  url: string;
  headerName: string;
  headerValue: string;
  variable: string;
  varDefined: string;
  varDefinedBg: string;
  varUndefined: string;
  varUndefinedBg: string;
  body: string;
  blockBorder: string;
  requestLineBg: string;
}

export interface EditorScheme {
  id: string;
  name: string;
  builtin: boolean;
  background: string;
  foreground: string;
  caret: string;
  selection: string;
  gutterBg: string;
  gutterFg: string;
  gutterBorder: string;
  /** 激活块左侧竖线 + Run 按钮颜色 */
  accent: string;
  /** Run 按钮悬停背景 */
  accentSoft: string;
  syntax: EditorSchemeSyntax;
}

/* ------------------------------------------------------------------ */
/* UI 主题基底                                                         */
/* ------------------------------------------------------------------ */

const darkBase: UiThemeVars = {
  'bg-base': '#1e1e1e',
  'bg-panel': '#252526',
  'bg-chrome': '#2d2d2d',
  'bg-hover': '#2a2d2e',
  'bg-active': '#37373d',
  'bg-input': '#2d2d2d',
  'bg-input-deep': '#1a1a1a',
  'bg-button': '#3c3c3c',
  'bg-button-hover': '#4c4c4c',
  'border': '#1a1a1a',
  'border-strong': '#3c3c3c',
  'border-block': '#333333',
  'fg': '#d4d4d4',
  'fg-secondary': '#cccccc',
  'fg-muted': '#858585',
  'fg-strong': '#ffffff',
  'accent': '#4ec9b0',
  'accent-soft': 'rgba(78, 201, 176, 0.15)',
  'focus': '#007acc',
  'focus-bg': '#094771',
  'focus-ring': 'rgba(3, 102, 214, 0.25)',
  'success': '#2ea043',
  'success-hover': '#3fb950',
  'success-fg': '#ffffff',
  'danger': '#f48771',
  'danger-strong': '#e81123',
  'danger-bg': '#3c2a2a',
  'danger-border': '#5a3a3a',
  'warning': '#e2c08d',
  'scrollbar': '#424242',
  'scrollbar-hover': '#4f4f4f',
  'splitter': '#2b2b2b',
  'splitter-hover': '#3b3b3b',
  'shadow': 'rgba(0, 0, 0, 0.4)',
  'overlay': 'rgba(0, 0, 0, 0.5)',
  'chip-safe-bg': '#1b3a1f',
  'chip-safe-fg': '#6dba6d',
  'chip-write-bg': '#1b2a3a',
  'chip-write-fg': '#6db4f0',
  'chip-danger-bg': '#3a1b1b',
  'chip-danger-fg': '#f48771',
  'chip-warn-bg': '#3a2a1b',
  'chip-warn-fg': '#e0c060',
  'chip-neutral-bg': '#2d2d2d',
  'chip-neutral-fg': '#858585',
  'syn-keyword': '#c586c0',
  'syn-url': '#9cdcfe',
  'syn-string': '#ce9178',
  'syn-comment': '#6a9955',
  'syn-number': '#b5cea8',
  'syn-bool': '#569cd6',
  'syn-tag': '#569cd6',
  'syn-attr': '#9cdcfe',
};

const lightBase: UiThemeVars = {
  'bg-base': '#ffffff',
  'bg-panel': '#f3f3f3',
  'bg-chrome': '#e8e8e8',
  'bg-hover': '#e6e6e6',
  'bg-active': '#d6d6d6',
  'bg-input': '#ffffff',
  'bg-input-deep': '#f7f7f7',
  'bg-button': '#e0e0e0',
  'bg-button-hover': '#d0d0d0',
  'border': '#d4d4d4',
  'border-strong': '#b8b8b8',
  'border-block': '#dddddd',
  'fg': '#1f1f1f',
  'fg-secondary': '#333333',
  'fg-muted': '#717171',
  'fg-strong': '#000000',
  'accent': '#008069',
  'accent-soft': 'rgba(0, 128, 105, 0.12)',
  'focus': '#0066b8',
  'focus-bg': '#c4dfeb',
  'focus-ring': 'rgba(0, 102, 184, 0.2)',
  'success': '#16825d',
  'success-hover': '#1a9e71',
  'success-fg': '#ffffff',
  'danger': '#a1260d',
  'danger-strong': '#c42b1c',
  'danger-bg': '#fbeae8',
  'danger-border': '#e5b3ad',
  'warning': '#bf8803',
  'scrollbar': '#c1c1c1',
  'scrollbar-hover': '#a8a8a8',
  'splitter': '#e5e5e5',
  'splitter-hover': '#d0d0d0',
  'shadow': 'rgba(0, 0, 0, 0.16)',
  'overlay': 'rgba(0, 0, 0, 0.25)',
  'chip-safe-bg': '#d8f0d8',
  'chip-safe-fg': '#116329',
  'chip-write-bg': '#d7e7f8',
  'chip-write-fg': '#0a5aa8',
  'chip-danger-bg': '#f8d7d5',
  'chip-danger-fg': '#a1260d',
  'chip-warn-bg': '#f5e6c0',
  'chip-warn-fg': '#7d5b00',
  'chip-neutral-bg': '#e8e8e8',
  'chip-neutral-fg': '#616161',
  'syn-keyword': '#af00db',
  'syn-url': '#0451a5',
  'syn-string': '#a31515',
  'syn-comment': '#008000',
  'syn-number': '#098658',
  'syn-bool': '#0000ff',
  'syn-tag': '#800000',
  'syn-attr': '#e50000',
};

/* ------------------------------------------------------------------ */
/* UI 主题列表                                                         */
/* ------------------------------------------------------------------ */

export const UI_THEMES: UiTheme[] = [
  {
    id: 'dark',
    name: '暗色',
    base: 'dark',
    builtin: true,
    source: '内置主题',
    vars: { ...darkBase },
    editorScheme: 'dark',
  },
  {
    id: 'light',
    name: '亮色',
    base: 'light',
    builtin: true,
    source: '内置主题',
    vars: { ...lightBase },
    editorScheme: 'light',
  },
  {
    id: 'github-light',
    name: 'GitHub Light',
    base: 'light',
    builtin: false,
    source: '开源 · GitHub Primer (MIT)',
    vars: {
      ...lightBase,
      'bg-base': '#ffffff',
      'bg-panel': '#f6f8fa',
      'bg-chrome': '#eaeef2',
      'bg-hover': '#eaeef2',
      'bg-active': '#dde3e9',
      'bg-input': '#f6f8fa',
      'bg-input-deep': '#f6f8fa',
      'bg-button': '#eaeef2',
      'bg-button-hover': '#dde3e9',
      'border': '#d0d7de',
      'border-strong': '#c4ccd4',
      'border-block': '#d0d7de',
      'fg': '#1f2328',
      'fg-secondary': '#32383f',
      'fg-muted': '#59636e',
      'fg-strong': '#0d1117',
      'accent': '#1a7f37',
      'accent-soft': 'rgba(26, 127, 55, 0.12)',
      'focus': '#0969da',
      'focus-bg': '#ddf4ff',
      'focus-ring': 'rgba(9, 105, 218, 0.3)',
      'success': '#1a7f37',
      'success-hover': '#218bff',
      'danger': '#d1242f',
      'danger-strong': '#cf222e',
      'danger-bg': '#ffebe9',
      'danger-border': '#ffabaf',
      'warning': '#9a6700',
      'chip-safe-bg': '#dafbe1',
      'chip-safe-fg': '#116329',
      'chip-write-bg': '#ddf4ff',
      'chip-write-fg': '#0550ae',
      'chip-danger-bg': '#ffebe9',
      'chip-danger-fg': '#cf222e',
      'chip-warn-bg': '#fff8c5',
      'chip-warn-fg': '#7d4e00',
      'chip-neutral-bg': '#eaeef2',
      'chip-neutral-fg': '#59636e',
      'syn-keyword': '#cf222e',
      'syn-url': '#0550ae',
      'syn-string': '#0a3069',
      'syn-comment': '#59636e',
      'syn-number': '#0550ae',
      'syn-bool': '#0550ae',
      'syn-tag': '#116329',
      'syn-attr': '#8250df',
    },
    editorScheme: 'github-light',
  },
  {
    id: 'github-dark',
    name: 'GitHub Dark',
    base: 'dark',
    builtin: false,
    source: '开源 · GitHub Primer (MIT)',
    vars: {
      ...darkBase,
      'bg-base': '#0d1117',
      'bg-panel': '#161b22',
      'bg-chrome': '#161b22',
      'bg-hover': '#21262d',
      'bg-active': '#2d333b',
      'bg-input': '#21262d',
      'bg-input-deep': '#0d1117',
      'bg-button': '#21262d',
      'bg-button-hover': '#30363d',
      'border': '#21262d',
      'border-strong': '#30363d',
      'border-block': '#30363d',
      'fg': '#e6edf3',
      'fg-secondary': '#d0d7de',
      'fg-muted': '#8b949e',
      'fg-strong': '#ffffff',
      'accent': '#3fb950',
      'accent-soft': 'rgba(63, 185, 80, 0.15)',
      'focus': '#58a6ff',
      'focus-bg': '#1f3a5f',
      'focus-ring': 'rgba(56, 139, 253, 0.4)',
      'success': '#238636',
      'success-hover': '#2ea043',
      'danger': '#f85149',
      'danger-strong': '#da3633',
      'danger-bg': '#3d1a1c',
      'danger-border': '#6e2b2e',
      'warning': '#d29922',
      'scrollbar': '#30363d',
      'scrollbar-hover': '#3d444d',
      'splitter': '#21262d',
      'splitter-hover': '#30363d',
      'chip-safe-bg': '#12261e',
      'chip-safe-fg': '#3fb950',
      'chip-write-bg': '#12233d',
      'chip-write-fg': '#58a6ff',
      'chip-danger-bg': '#3d1a1c',
      'chip-danger-fg': '#f85149',
      'chip-warn-bg': '#33280e',
      'chip-warn-fg': '#d29922',
      'chip-neutral-bg': '#21262d',
      'chip-neutral-fg': '#8b949e',
      'syn-keyword': '#ff7b72',
      'syn-url': '#79c0ff',
      'syn-string': '#a5d6ff',
      'syn-comment': '#8b949e',
      'syn-number': '#79c0ff',
      'syn-bool': '#79c0ff',
      'syn-tag': '#7ee787',
      'syn-attr': '#d2a8ff',
    },
    editorScheme: 'github-dark',
  },
  {
    id: 'dracula',
    name: 'Dracula',
    base: 'dark',
    builtin: false,
    source: '开源 · Dracula Theme (MIT)',
    vars: {
      ...darkBase,
      'bg-base': '#282a36',
      'bg-panel': '#21222c',
      'bg-chrome': '#191a21',
      'bg-hover': '#343746',
      'bg-active': '#44475a',
      'bg-input': '#343746',
      'bg-input-deep': '#1e1f29',
      'bg-button': '#44475a',
      'bg-button-hover': '#565973',
      'border': '#191a21',
      'border-strong': '#44475a',
      'border-block': '#44475a',
      'fg': '#f8f8f2',
      'fg-secondary': '#e2e2dc',
      'fg-muted': '#6272a4',
      'fg-strong': '#ffffff',
      'accent': '#50fa7b',
      'accent-soft': 'rgba(80, 250, 123, 0.15)',
      'focus': '#bd93f9',
      'focus-bg': '#44475a',
      'focus-ring': 'rgba(189, 147, 249, 0.35)',
      'success': '#50fa7b',
      'success-hover': '#69fb90',
      'success-fg': '#21222c',
      'danger': '#ff5555',
      'danger-strong': '#ff5555',
      'danger-bg': '#3d2430',
      'danger-border': '#6e3a4d',
      'warning': '#f1fa8c',
      'scrollbar': '#44475a',
      'scrollbar-hover': '#565973',
      'splitter': '#21222c',
      'splitter-hover': '#343746',
      'chip-safe-bg': '#1e3a2a',
      'chip-safe-fg': '#50fa7b',
      'chip-write-bg': '#2c3150',
      'chip-write-fg': '#8be9fd',
      'chip-danger-bg': '#3d2430',
      'chip-danger-fg': '#ff5555',
      'chip-warn-bg': '#3d3a1e',
      'chip-warn-fg': '#f1fa8c',
      'chip-neutral-bg': '#343746',
      'chip-neutral-fg': '#9aa0c3',
      'syn-keyword': '#ff79c6',
      'syn-url': '#8be9fd',
      'syn-string': '#f1fa8c',
      'syn-comment': '#6272a4',
      'syn-number': '#bd93f9',
      'syn-bool': '#bd93f9',
      'syn-tag': '#ff79c6',
      'syn-attr': '#50fa7b',
    },
    editorScheme: 'dracula',
  },
  {
    id: 'solarized-light',
    name: 'Solarized Light',
    base: 'light',
    builtin: false,
    source: '开源 · Solarized (MIT)',
    vars: {
      ...lightBase,
      'bg-base': '#fdf6e3',
      'bg-panel': '#eee8d5',
      'bg-chrome': '#eee8d5',
      'bg-hover': '#e9e1c8',
      'bg-active': '#e0d8bc',
      'bg-input': '#fdf6e3',
      'bg-input-deep': '#f7f0dd',
      'bg-button': '#e6dfc9',
      'bg-button-hover': '#d9d0b4',
      'border': '#ddd6c1',
      'border-strong': '#c9c0a5',
      'border-block': '#ddd6c1',
      'fg': '#586e75',
      'fg-secondary': '#4d646c',
      'fg-muted': '#93a1a1',
      'fg-strong': '#073642',
      'accent': '#2aa198',
      'accent-soft': 'rgba(42, 161, 152, 0.15)',
      'focus': '#268bd2',
      'focus-bg': '#d5e7f2',
      'focus-ring': 'rgba(38, 139, 210, 0.25)',
      'success': '#859900',
      'success-hover': '#98ad0d',
      'danger': '#dc322f',
      'danger-strong': '#dc322f',
      'danger-bg': '#f6e0de',
      'danger-border': '#e0b4b1',
      'warning': '#b58900',
      'scrollbar': '#cfc7ad',
      'scrollbar-hover': '#bdb494',
      'splitter': '#eee8d5',
      'splitter-hover': '#ddd6c1',
      'chip-safe-bg': '#e8ecc8',
      'chip-safe-fg': '#5f6b02',
      'chip-write-bg': '#d5e7f2',
      'chip-write-fg': '#1a6091',
      'chip-danger-bg': '#f6e0de',
      'chip-danger-fg': '#a5241f',
      'chip-warn-bg': '#f0e6c0',
      'chip-warn-fg': '#815f00',
      'chip-neutral-bg': '#eee8d5',
      'chip-neutral-fg': '#839496',
      'syn-keyword': '#859900',
      'syn-url': '#268bd2',
      'syn-string': '#2aa198',
      'syn-comment': '#93a1a1',
      'syn-number': '#d33682',
      'syn-bool': '#d33682',
      'syn-tag': '#268bd2',
      'syn-attr': '#b58900',
    },
    editorScheme: 'solarized-light',
  },
  {
    id: 'nord',
    name: 'Nord',
    base: 'dark',
    builtin: false,
    source: '开源 · Nord (MIT)',
    vars: {
      ...darkBase,
      'bg-base': '#2e3440',
      'bg-panel': '#3b4252',
      'bg-chrome': '#353c4a',
      'bg-hover': '#434c5e',
      'bg-active': '#4c566a',
      'bg-input': '#3b4252',
      'bg-input-deep': '#2a2f3a',
      'bg-button': '#434c5e',
      'bg-button-hover': '#4c566a',
      'border': '#2a2f3a',
      'border-strong': '#4c566a',
      'border-block': '#434c5e',
      'fg': '#d8dee9',
      'fg-secondary': '#c8d0e0',
      'fg-muted': '#7b88a1',
      'fg-strong': '#eceff4',
      'accent': '#88c0d0',
      'accent-soft': 'rgba(136, 192, 208, 0.15)',
      'focus': '#81a1c1',
      'focus-bg': '#4c566a',
      'focus-ring': 'rgba(129, 161, 193, 0.35)',
      'success': '#a3be8c',
      'success-hover': '#b2caa0',
      'success-fg': '#2e3440',
      'danger': '#bf616a',
      'danger-strong': '#bf616a',
      'danger-bg': '#453239',
      'danger-border': '#6e4a52',
      'warning': '#ebcb8b',
      'scrollbar': '#4c566a',
      'scrollbar-hover': '#5e6880',
      'splitter': '#3b4252',
      'splitter-hover': '#434c5e',
      'chip-safe-bg': '#39423a',
      'chip-safe-fg': '#a3be8c',
      'chip-write-bg': '#384152',
      'chip-write-fg': '#88c0d0',
      'chip-danger-bg': '#453239',
      'chip-danger-fg': '#bf616a',
      'chip-warn-bg': '#453e2c',
      'chip-warn-fg': '#ebcb8b',
      'chip-neutral-bg': '#3b4252',
      'chip-neutral-fg': '#9aa5bd',
      'syn-keyword': '#81a1c1',
      'syn-url': '#88c0d0',
      'syn-string': '#a3be8c',
      'syn-comment': '#616e88',
      'syn-number': '#b48ead',
      'syn-bool': '#b48ead',
      'syn-tag': '#81a1c1',
      'syn-attr': '#8fbcbb',
    },
    editorScheme: 'nord',
  },
  {
    id: 'one-dark-pro',
    name: 'One Dark Pro',
    base: 'dark',
    builtin: false,
    source: '开源 · Atom One Dark (MIT)',
    vars: {
      ...darkBase,
      'bg-base': '#282c34',
      'bg-panel': '#21252b',
      'bg-chrome': '#21252b',
      'bg-hover': '#2c313a',
      'bg-active': '#3a404d',
      'bg-input': '#2c313a',
      'bg-input-deep': '#1f232a',
      'bg-button': '#3a404d',
      'bg-button-hover': '#464d5d',
      'border': '#181a1f',
      'border-strong': '#3a404d',
      'border-block': '#3e4451',
      'fg': '#abb2bf',
      'fg-secondary': '#9da5b4',
      'fg-muted': '#5c6370',
      'fg-strong': '#e6e8ee',
      'accent': '#98c379',
      'accent-soft': 'rgba(152, 195, 121, 0.15)',
      'focus': '#61afef',
      'focus-bg': '#3e4451',
      'focus-ring': 'rgba(97, 175, 239, 0.35)',
      'success': '#98c379',
      'success-hover': '#a9cf8d',
      'success-fg': '#282c34',
      'danger': '#e06c75',
      'danger-strong': '#e06c75',
      'danger-bg': '#40303a',
      'danger-border': '#6b4a53',
      'warning': '#e5c07b',
      'scrollbar': '#4b5261',
      'scrollbar-hover': '#5c6370',
      'splitter': '#181a1f',
      'splitter-hover': '#3a404d',
      'chip-safe-bg': '#313a2e',
      'chip-safe-fg': '#98c379',
      'chip-write-bg': '#2d3a4a',
      'chip-write-fg': '#61afef',
      'chip-danger-bg': '#40303a',
      'chip-danger-fg': '#e06c75',
      'chip-warn-bg': '#40392a',
      'chip-warn-fg': '#e5c07b',
      'chip-neutral-bg': '#2c313a',
      'chip-neutral-fg': '#8b93a3',
      'syn-keyword': '#c678dd',
      'syn-url': '#61afef',
      'syn-string': '#98c379',
      'syn-comment': '#5c6370',
      'syn-number': '#d19a66',
      'syn-bool': '#d19a66',
      'syn-tag': '#e06c75',
      'syn-attr': '#d19a66',
    },
    editorScheme: 'one-dark-pro',
  },
];

/* ------------------------------------------------------------------ */
/* 编辑器 Color Scheme                                                 */
/* ------------------------------------------------------------------ */

export const EDITOR_SCHEMES: EditorScheme[] = [
  {
    id: 'dark',
    name: 'Dark（VSCode Dark+）',
    builtin: true,
    background: '#1e1e1e',
    foreground: '#d4d4d4',
    caret: '#aeafad',
    selection: '#264f78',
    gutterBg: '#252526',
    gutterFg: '#858585',
    gutterBorder: '#1a1a1a',
    accent: '#4ec9b0',
    accentSoft: 'rgba(78, 201, 176, 0.15)',
    syntax: {
      separator: '#808080',
      separatorName: '#dcdcaa',
      separatorComment: '#808080',
      comment: '#808080',
      method: '#c586c0',
      url: '#9cdcfe',
      headerName: '#c586c0',
      headerValue: '#ce9178',
      variable: '#dcdcaa',
      varDefined: '#73c991',
      varDefinedBg: 'rgba(115, 201, 145, 0.1)',
      varUndefined: '#f48771',
      varUndefinedBg: 'rgba(244, 135, 113, 0.1)',
      body: '#d4d4d4',
      blockBorder: '#333333',
      requestLineBg: '#252526',
    },
  },
  {
    id: 'light',
    name: 'Light（VSCode Light+）',
    builtin: true,
    background: '#ffffff',
    foreground: '#1f1f1f',
    caret: '#000000',
    selection: '#add6ff',
    gutterBg: '#ffffff',
    gutterFg: '#237893',
    gutterBorder: '#e5e5e5',
    accent: '#008069',
    accentSoft: 'rgba(0, 128, 105, 0.12)',
    syntax: {
      separator: '#808080',
      separatorName: '#795e26',
      separatorComment: '#008000',
      comment: '#008000',
      method: '#af00db',
      url: '#0451a5',
      headerName: '#af00db',
      headerValue: '#a31515',
      variable: '#795e26',
      varDefined: '#16825d',
      varDefinedBg: 'rgba(22, 130, 93, 0.12)',
      varUndefined: '#b5342e',
      varUndefinedBg: 'rgba(181, 52, 46, 0.1)',
      body: '#1f1f1f',
      blockBorder: '#dddddd',
      requestLineBg: '#f3f3f3',
    },
  },
  {
    id: 'dracula',
    name: 'Dracula',
    builtin: false,
    background: '#282a36',
    foreground: '#f8f8f2',
    caret: '#f8f8f2',
    selection: '#44475a',
    gutterBg: '#282a36',
    gutterFg: '#6272a4',
    gutterBorder: '#343746',
    accent: '#50fa7b',
    accentSoft: 'rgba(80, 250, 123, 0.15)',
    syntax: {
      separator: '#6272a4',
      separatorName: '#f1fa8c',
      separatorComment: '#6272a4',
      comment: '#6272a4',
      method: '#ff79c6',
      url: '#8be9fd',
      headerName: '#ff79c6',
      headerValue: '#f1fa8c',
      variable: '#ffb86c',
      varDefined: '#50fa7b',
      varDefinedBg: 'rgba(80, 250, 123, 0.12)',
      varUndefined: '#ff5555',
      varUndefinedBg: 'rgba(255, 85, 85, 0.12)',
      body: '#f8f8f2',
      blockBorder: '#44475a',
      requestLineBg: '#32344a',
    },
  },
  {
    id: 'monokai',
    name: 'Monokai',
    builtin: false,
    background: '#272822',
    foreground: '#f8f8f2',
    caret: '#f8f8f2',
    selection: '#49483e',
    gutterBg: '#272822',
    gutterFg: '#90918b',
    gutterBorder: '#3b3c35',
    accent: '#a6e22e',
    accentSoft: 'rgba(166, 226, 46, 0.15)',
    syntax: {
      separator: '#75715e',
      separatorName: '#e6db74',
      separatorComment: '#75715e',
      comment: '#75715e',
      method: '#f92672',
      url: '#66d9ef',
      headerName: '#f92672',
      headerValue: '#e6db74',
      variable: '#e6db74',
      varDefined: '#a6e22e',
      varDefinedBg: 'rgba(166, 226, 46, 0.12)',
      varUndefined: '#f92672',
      varUndefinedBg: 'rgba(249, 38, 114, 0.12)',
      body: '#f8f8f2',
      blockBorder: '#49483e',
      requestLineBg: '#2e2f28',
    },
  },
  {
    id: 'solarized-light',
    name: 'Solarized Light',
    builtin: false,
    background: '#fdf6e3',
    foreground: '#657b83',
    caret: '#657b83',
    selection: '#eee8d5',
    gutterBg: '#fdf6e3',
    gutterFg: '#93a1a1',
    gutterBorder: '#eee8d5',
    accent: '#2aa198',
    accentSoft: 'rgba(42, 161, 152, 0.15)',
    syntax: {
      separator: '#93a1a1',
      separatorName: '#b58900',
      separatorComment: '#93a1a1',
      comment: '#93a1a1',
      method: '#859900',
      url: '#268bd2',
      headerName: '#859900',
      headerValue: '#2aa198',
      variable: '#b58900',
      varDefined: '#859900',
      varDefinedBg: 'rgba(133, 153, 0, 0.12)',
      varUndefined: '#dc322f',
      varUndefinedBg: 'rgba(220, 50, 47, 0.1)',
      body: '#657b83',
      blockBorder: '#eee8d5',
      requestLineBg: '#f5eedb',
    },
  },
  {
    id: 'nord',
    name: 'Nord',
    builtin: false,
    background: '#2e3440',
    foreground: '#d8dee9',
    caret: '#d8dee9',
    selection: '#434c5e',
    gutterBg: '#2e3440',
    gutterFg: '#616e88',
    gutterBorder: '#3b4252',
    accent: '#a3be8c',
    accentSoft: 'rgba(163, 190, 140, 0.15)',
    syntax: {
      separator: '#616e88',
      separatorName: '#ebcb8b',
      separatorComment: '#616e88',
      comment: '#616e88',
      method: '#81a1c1',
      url: '#88c0d0',
      headerName: '#81a1c1',
      headerValue: '#a3be8c',
      variable: '#ebcb8b',
      varDefined: '#a3be8c',
      varDefinedBg: 'rgba(163, 190, 140, 0.12)',
      varUndefined: '#bf616a',
      varUndefinedBg: 'rgba(191, 97, 106, 0.12)',
      body: '#d8dee9',
      blockBorder: '#3b4252',
      requestLineBg: '#3b4252',
    },
  },
  {
    id: 'github-light',
    name: 'GitHub Light',
    builtin: false,
    background: '#ffffff',
    foreground: '#1f2328',
    caret: '#1f2328',
    selection: '#c8e1ff',
    gutterBg: '#ffffff',
    gutterFg: '#59636e',
    gutterBorder: '#d0d7de',
    accent: '#1a7f37',
    accentSoft: 'rgba(26, 127, 55, 0.12)',
    syntax: {
      separator: '#59636e',
      separatorName: '#953800',
      separatorComment: '#59636e',
      comment: '#59636e',
      method: '#cf222e',
      url: '#0550ae',
      headerName: '#8250df',
      headerValue: '#0a3069',
      variable: '#953800',
      varDefined: '#116329',
      varDefinedBg: 'rgba(26, 127, 55, 0.12)',
      varUndefined: '#cf222e',
      varUndefinedBg: 'rgba(209, 36, 47, 0.1)',
      body: '#1f2328',
      blockBorder: '#d0d7de',
      requestLineBg: '#f6f8fa',
    },
  },
  {
    id: 'github-dark',
    name: 'GitHub Dark',
    builtin: false,
    background: '#0d1117',
    foreground: '#e6edf3',
    caret: '#e6edf3',
    selection: '#264f78',
    gutterBg: '#0d1117',
    gutterFg: '#6e7681',
    gutterBorder: '#21262d',
    accent: '#3fb950',
    accentSoft: 'rgba(63, 185, 80, 0.15)',
    syntax: {
      separator: '#8b949e',
      separatorName: '#d29922',
      separatorComment: '#8b949e',
      comment: '#8b949e',
      method: '#ff7b72',
      url: '#79c0ff',
      headerName: '#d2a8ff',
      headerValue: '#a5d6ff',
      variable: '#ffa657',
      varDefined: '#3fb950',
      varDefinedBg: 'rgba(63, 185, 80, 0.12)',
      varUndefined: '#f85149',
      varUndefinedBg: 'rgba(248, 81, 73, 0.12)',
      body: '#e6edf3',
      blockBorder: '#30363d',
      requestLineBg: '#161b22',
    },
  },
  {
    id: 'one-dark-pro',
    name: 'One Dark Pro',
    builtin: false,
    background: '#282c34',
    foreground: '#abb2bf',
    caret: '#528bff',
    selection: '#3e4451',
    gutterBg: '#282c34',
    gutterFg: '#5c6370',
    gutterBorder: '#2c313c',
    accent: '#98c379',
    accentSoft: 'rgba(152, 195, 121, 0.15)',
    syntax: {
      separator: '#5c6370',
      separatorName: '#e5c07b',
      separatorComment: '#5c6370',
      comment: '#5c6370',
      method: '#c678dd',
      url: '#61afef',
      headerName: '#c678dd',
      headerValue: '#98c379',
      variable: '#e5c07b',
      varDefined: '#98c379',
      varDefinedBg: 'rgba(152, 195, 121, 0.12)',
      varUndefined: '#e06c75',
      varUndefinedBg: 'rgba(224, 108, 117, 0.12)',
      body: '#abb2bf',
      blockBorder: '#3e4451',
      requestLineBg: '#2c313a',
    },
  },
];

/* ------------------------------------------------------------------ */
/* 查询辅助                                                            */
/* ------------------------------------------------------------------ */

export const DEFAULT_THEME_ID = 'dark';
export const DEFAULT_EDITOR_SCHEME_ID = 'dark';
/** editorSchemeId 的特殊值 — 跟随当前 UI 主题 */
export const EDITOR_SCHEME_FOLLOW = 'follow';

export function getTheme(id: string): UiTheme {
  return UI_THEMES.find((t) => t.id === id) ?? UI_THEMES[0];
}

export function getEditorScheme(id: string): EditorScheme {
  return EDITOR_SCHEMES.find((s) => s.id === id) ?? EDITOR_SCHEMES[0];
}

/** 将主题变量应用到 documentElement（`--` 前缀自动补齐） */
export function applyThemeVars(vars: UiThemeVars) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(`--${key}`, value);
  }
}
