/**
 * 快捷键方案定义 — 内置 Windows 风格，另提供 VSCode / JetBrains 风格方案。
 *
 * 绑定以 CodeMirror keymap 格式存储（如 `Mod-Enter`、`Mod-s`），
 * 其中 `Mod` 在 Windows/Linux 上等价于 `Ctrl`，在 macOS 上为 `Cmd`。
 * `formatBinding()` 将其转换为界面展示格式（如 `Ctrl+Enter`）。
 */

export type KeymapSchemeId = 'windows' | 'vscode' | 'jetbrains';

/** 应用级快捷键动作清单 */
export type ShortcutAction =
  | 'runRequest'
  | 'save'
  | 'saveAs'
  | 'newFile'
  | 'openFile'
  | 'toggleSidebar'
  | 'undo'
  | 'redo'
  | 'find'
  | 'zoomIn'
  | 'zoomOut'
  | 'resetZoom';

export interface ShortcutActionDef {
  id: ShortcutAction;
  /** 展示名称 */
  label: string;
  /** 分组（用于设置页表格分组展示） */
  group: '通用' | '文件' | '编辑' | '视图';
}

/** 动作清单（设置页按此顺序渲染） */
export const SHORTCUT_ACTIONS: ShortcutActionDef[] = [
  { id: 'runRequest', label: '运行请求', group: '通用' },
  { id: 'save', label: '保存', group: '通用' },
  { id: 'saveAs', label: '另存为', group: '通用' },
  { id: 'find', label: '查找', group: '通用' },
  { id: 'newFile', label: '新建文件', group: '文件' },
  { id: 'openFile', label: '打开文件', group: '文件' },
  { id: 'toggleSidebar', label: '切换侧边栏', group: '视图' },
  { id: 'zoomIn', label: '放大', group: '视图' },
  { id: 'zoomOut', label: '缩小', group: '视图' },
  { id: 'resetZoom', label: '重置缩放', group: '视图' },
  { id: 'undo', label: '撤销', group: '编辑' },
  { id: 'redo', label: '重做', group: '编辑' },
];

/** 一套快捷键方案的全部绑定（CodeMirror 格式） */
export type KeymapBindings = Record<ShortcutAction, string>;

export interface KeymapScheme {
  id: KeymapSchemeId;
  name: string;
  /** 是否为系统内置方案 */
  builtin: boolean;
  description: string;
  bindings: KeymapBindings;
}

/** Windows 风格（内置） */
const windowsBindings: KeymapBindings = {
  runRequest: 'Mod-Enter',
  save: 'Mod-s',
  saveAs: 'Mod-Shift-s',
  newFile: 'Mod-n',
  openFile: 'Mod-o',
  toggleSidebar: 'Mod-b',
  undo: 'Mod-z',
  redo: 'Mod-y',
  find: 'Mod-f',
  zoomIn: 'Mod-=',
  zoomOut: 'Mod--',
  resetZoom: 'Mod-0',
};

/** VSCode 风格 */
const vscodeBindings: KeymapBindings = {
  ...windowsBindings,
  runRequest: 'Mod-Alt-r',
  redo: 'Mod-Shift-z',
};

/** JetBrains（IntelliJ IDEA）风格 */
const jetbrainsBindings: KeymapBindings = {
  ...windowsBindings,
  runRequest: 'Mod-Enter',
  saveAs: 'Mod-Shift-Alt-s',
  newFile: 'Mod-Alt-n',
  openFile: 'Mod-Shift-o',
  toggleSidebar: 'Alt-1',
  redo: 'Mod-Shift-z',
};

export const KEYMAPS: Record<KeymapSchemeId, KeymapScheme> = {
  windows: {
    id: 'windows',
    name: 'Windows（内置）',
    builtin: true,
    description: '系统内置的 Windows 风格快捷键',
    bindings: windowsBindings,
  },
  vscode: {
    id: 'vscode',
    name: 'VSCode 风格',
    builtin: false,
    description: '贴近 Visual Studio Code 的快捷键习惯',
    bindings: vscodeBindings,
  },
  jetbrains: {
    id: 'jetbrains',
    name: 'JetBrains 风格',
    builtin: false,
    description: '贴近 IntelliJ IDEA / WebStorm 的快捷键习惯',
    bindings: jetbrainsBindings,
  },
};

export const KEYMAP_SCHEME_LIST: KeymapScheme[] = [
  KEYMAPS.windows,
  KEYMAPS.vscode,
  KEYMAPS.jetbrains,
];

export const DEFAULT_KEYMAP_SCHEME: KeymapSchemeId = 'windows';

export function getKeymapScheme(id: KeymapSchemeId): KeymapScheme {
  return KEYMAPS[id] ?? KEYMAPS.windows;
}

/**
 * 将 CodeMirror 格式的绑定转换为展示格式。
 * `Mod-Enter` → `Ctrl+Enter`，`Mod--` → `Ctrl+-`，`Mod-Shift-Alt-s` → `Ctrl+Shift+Alt+S`。
 */
export function formatBinding(binding: string): string {
  const parts: string[] = [];
  let rest = binding;
  for (;;) {
    if (rest.startsWith('Mod-')) {
      parts.push('Ctrl');
      rest = rest.slice(4);
    } else if (rest.startsWith('Shift-')) {
      parts.push('Shift');
      rest = rest.slice(6);
    } else if (rest.startsWith('Alt-')) {
      parts.push('Alt');
      rest = rest.slice(4);
    } else if (rest.startsWith('Ctrl-')) {
      parts.push('Ctrl');
      rest = rest.slice(5);
    } else {
      break;
    }
  }
  if (rest.length > 0) {
    // 单字符按键（如 s、0、-）转大写展示；多字符按键（Enter、Escape）原样展示。
    parts.push(rest.length === 1 ? rest.toUpperCase() : rest);
  }
  return parts.join('+');
}

/** 取某个方案下某个动作的展示文本 */
export function formatActionBinding(schemeId: KeymapSchemeId, action: ShortcutAction): string {
  return formatBinding(getKeymapScheme(schemeId).bindings[action]);
}

export interface ParsedBinding {
  /** `Mod`：Windows/Linux 上为 Ctrl，macOS 上为 Cmd */
  mod: boolean;
  shift: boolean;
  alt: boolean;
  /** 显式 `Ctrl-`（与 `Mod-` 合并匹配） */
  ctrl: boolean;
  /** 基础键（如 `s`、`-`、`Enter`） */
  key: string;
}

/**
 * 解析 CodeMirror 格式绑定串（如 `Mod-Shift-s`、`Mod--`）。
 * 逐个剥离修饰符前缀，剩余部分为基础键。非法绑定返回 `null`。
 */
export function parseBinding(binding: string): ParsedBinding | null {
  const out: ParsedBinding = { mod: false, shift: false, alt: false, ctrl: false, key: '' };
  let rest = binding;
  for (;;) {
    if (rest.startsWith('Mod-')) {
      out.mod = true;
      rest = rest.slice(4);
    } else if (rest.startsWith('Shift-')) {
      out.shift = true;
      rest = rest.slice(6);
    } else if (rest.startsWith('Alt-')) {
      out.alt = true;
      rest = rest.slice(4);
    } else if (rest.startsWith('Ctrl-')) {
      out.ctrl = true;
      rest = rest.slice(5);
    } else {
      break;
    }
  }
  if (rest.length === 0) return null;
  out.key = rest;
  return out;
}

/**
 * 判断键盘事件是否匹配绑定串（修饰符精确匹配，按键忽略大小写）。
 * 输入法合成中（isComposing）一律不匹配。
 */
export function eventMatchesBinding(e: KeyboardEvent, binding: string): boolean {
  if (e.isComposing || e.key === 'Dead' || e.key === '') return false;
  const b = parseBinding(binding);
  if (!b) return false;
  const ctrlLike = e.ctrlKey || e.metaKey;
  if (ctrlLike !== (b.mod || b.ctrl)) return false;
  if (e.shiftKey !== b.shift) return false;
  if (e.altKey !== b.alt) return false;
  return e.key.toLowerCase() === b.key.toLowerCase();
}
