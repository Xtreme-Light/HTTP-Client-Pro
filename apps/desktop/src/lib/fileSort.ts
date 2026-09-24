import type { FileEntry } from '../stores/workspace';

/** 排序字段：文件名称 / 创建时间 / 修改时间 */
export type FileSortField = 'name' | 'created' | 'modified';
/** 排序方向：升序 / 降序 */
export type FileSortOrder = 'asc' | 'desc';

export interface FileSort {
  field: FileSortField;
  order: FileSortOrder;
}

/** 默认排序：文件名称降序（如 `20260924xxx.http` 排在 `20260923.http` 前面） */
export const DEFAULT_FILE_SORT: FileSort = { field: 'name', order: 'desc' };

export const FILE_SORT_FIELDS: { id: FileSortField; label: string }[] = [
  { id: 'name', label: '名称' },
  { id: 'modified', label: '修改时间' },
  { id: 'created', label: '创建时间' },
];

const STORAGE_KEY = 'http-client-pro:file-sort';

/** 数字感知比较：`a-2.http` 排在 `a-10.http` 之前，日期前缀名按数值大小比较 */
const COLLATOR = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function compareName(a: FileEntry, b: FileEntry): number {
  const r = COLLATOR.compare(a.name, b.name);
  // 忽略大小写后仍相同（如 `A.http` / `a.http`）时用原始字符串兜底，保证顺序稳定
  if (r !== 0) return r;
  return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
}

function compareField(a: FileEntry, b: FileEntry, field: FileSortField): number {
  if (field === 'name') return compareName(a, b);
  const key = field === 'created' ? 'createdAt' : 'modifiedAt';
  const diff = (a[key] ?? 0) - (b[key] ?? 0);
  // 时间相同（或缺失时间戳）时退回名称，避免出现随机的抖动顺序
  return diff !== 0 ? diff : compareName(a, b);
}

/**
 * 按指定方式排序目录条目 — 目录始终排在文件之前，组内按字段与方向排序。
 * 返回新数组，不修改入参。
 */
export function sortFileEntries<T extends FileEntry>(entries: readonly T[], sort: FileSort): T[] {
  const dir = sort.order === 'desc' ? -1 : 1;
  return [...entries].sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
    return compareField(a, b, sort.field) * dir;
  });
}

/** 从 localStorage 读取排序偏好（缺失或非法时回退默认值） */
export function loadFileSort(): FileSort {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FILE_SORT };
    const parsed = JSON.parse(raw) as Partial<FileSort>;
    return {
      field: FILE_SORT_FIELDS.some((f) => f.id === parsed?.field)
        ? (parsed.field as FileSortField)
        : DEFAULT_FILE_SORT.field,
      order: parsed?.order === 'asc' || parsed?.order === 'desc'
        ? parsed.order
        : DEFAULT_FILE_SORT.order,
    };
  } catch {
    return { ...DEFAULT_FILE_SORT };
  }
}

export function saveFileSort(sort: FileSort): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(sort));
  } catch { /* ignore */ }
}
