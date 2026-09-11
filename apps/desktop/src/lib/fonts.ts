/**
 * 界面字体选择与检测。
 * 通过 canvas 文本宽度测量判断系统是否安装了某个字体：
 * 以 `"<候选>", monospace` 渲染测试文本，若宽度与三个基线字体
 * （monospace / sans-serif / serif）都不同，则认为字体已安装。
 */

export interface SystemFontDef {
  name: string;
  /** 常见平台提示 */
  hint: string;
}

/** 常见系统字体候选（设置页下拉列表） */
export const SYSTEM_FONTS: SystemFontDef[] = [
  { name: 'Segoe UI', hint: 'Windows' },
  { name: 'Segoe UI Variable', hint: 'Windows 11' },
  { name: 'Microsoft YaHei UI', hint: 'Windows' },
  { name: 'Microsoft YaHei', hint: 'Windows' },
  { name: 'SimHei', hint: 'Windows' },
  { name: 'PingFang SC', hint: 'macOS' },
  { name: 'Hiragino Sans GB', hint: 'macOS' },
  { name: 'Helvetica Neue', hint: 'macOS' },
  { name: 'Arial', hint: 'Windows / macOS' },
  { name: 'Verdana', hint: 'Windows / macOS' },
  { name: 'Tahoma', hint: 'Windows / macOS' },
  { name: 'Roboto', hint: 'Android / Linux' },
  { name: 'Noto Sans', hint: 'Linux' },
  { name: 'Noto Sans SC', hint: 'Linux' },
  { name: 'Ubuntu', hint: 'Ubuntu' },
  { name: 'Cantarell', hint: 'GNOME' },
  { name: 'WenQuanYi Micro Hei', hint: 'Linux' },
  { name: 'JetBrains Mono', hint: '等宽' },
  { name: 'Fira Code', hint: '等宽' },
  { name: 'Cascadia Code', hint: 'Windows' },
  { name: 'Consolas', hint: 'Windows' },
  { name: 'Maple Mono', hint: '等宽' },
];

/** 未自定义字体时的默认 UI 字体栈 */
export const DEFAULT_UI_FONT_STACK =
  "-apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif";

/** 未自定义字体时的默认编辑器/等宽字体栈 */
export const DEFAULT_EDITOR_FONT_STACK =
  '"Maple Mono", "Maple Mono NF", ui-monospace, SFMono-Regular, Menlo, monospace';

const TEST_TEXT = 'mmmmmmmmmmlliWWI-0123456789';
const TEST_FONT_SIZE = '72px';
const BASELINE_FONTS = ['monospace', 'sans-serif', 'serif'];

let baselineWidths: number[] | null = null;
const availabilityCache = new Map<string, boolean>();

const GENERIC_FAMILIES = new Set([
  'sans-serif',
  'serif',
  'monospace',
  'system-ui',
  'ui-sans-serif',
  'ui-serif',
  'ui-monospace',
  'cursive',
  'fantasy',
]);

/** 为 CSS / canvas 安全地引用字体名（含空格等特殊字符时加引号） */
export function quoteCssFont(name: string): string {
  const t = name.trim();
  if (!t) return '';
  if (GENERIC_FAMILIES.has(t)) return t;
  if (/^[A-Za-z][A-Za-z0-9._-]*$/.test(t)) return t;
  return `'${t.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
}

/** 拆分逗号分隔的字体列表（去除空白与外层引号） */
export function splitFontList(value: string): string[] {
  return value
    .split(',')
    .map((s) => s.trim().replace(/^['"]/, '').replace(/['"]$/, '').trim())
    .filter((s) => s.length > 0);
}

function measureWidth(fontFamily: string): number {
  try {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    if (!ctx) return -1;
    ctx.font = `${TEST_FONT_SIZE} ${fontFamily}`;
    return ctx.measureText(TEST_TEXT).width;
  } catch {
    return -1;
  }
}

/** 检测系统中是否安装了某个字体（非浏览器环境返回 false） */
export function isFontAvailable(font: string): boolean {
  if (typeof document === 'undefined') return false;
  const key = font.trim();
  if (!key) return false;
  const cached = availabilityCache.get(key);
  if (cached !== undefined) return cached;
  if (!baselineWidths) {
    baselineWidths = BASELINE_FONTS.map((f) => measureWidth(f));
  }
  const width = measureWidth(`${quoteCssFont(key)}, monospace`);
  const available = width >= 0 && baselineWidths.some((w) => w >= 0 && w !== width);
  availabilityCache.set(key, available);
  return available;
}

/**
 * 构建字体栈：主字体 + fallback 列表，末尾保证有通用族兜底。
 * 两者均为空时返回 defaultStack。
 */
function buildFontStack(
  primary: string,
  fallback: string,
  defaultStack: string,
  tailFamily: string,
): string {
  const names = [...splitFontList(primary), ...splitFontList(fallback)];
  const parts = names.map((n) => (GENERIC_FAMILIES.has(n) ? n : quoteCssFont(n))).filter(Boolean);
  if (parts.length === 0) return defaultStack;
  if (!GENERIC_FAMILIES.has(parts[parts.length - 1])) {
    parts.push(tailFamily);
  }
  return parts.join(', ');
}

/** 构建 UI 字体栈（空则回退到默认无衬线栈） */
export function buildUiFontStack(primary: string, fallback: string): string {
  return buildFontStack(primary, fallback, DEFAULT_UI_FONT_STACK, 'sans-serif');
}

/** 构建编辑器/Console 字体栈（空则回退到内置等宽栈） */
export function buildEditorFontStack(primary: string, fallback: string): string {
  return buildFontStack(primary, fallback, DEFAULT_EDITOR_FONT_STACK, 'monospace');
}
