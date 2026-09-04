import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { DispatchResponse } from '../types/http';

export interface HistoryItem {
  id: number;
  ts: number;
  method: string;
  target: string;
  status: number;
  elapsedMs: number;
  source: string;
  /** 完整的响应快照 — 点击历史记录时恢复现场 */
  response: DispatchResponse | null;
}

const STORAGE_KEY = 'http-client-pro:history';
const MAX_ITEMS = 50;

/** 序列化 history items — 可独立测试 */
export function serialize(items: HistoryItem[]): string {
  return JSON.stringify(items);
}

/** 反序列化 — 格式错误时返回空数组 */
export function deserialize(raw: string | null): HistoryItem[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((x) => x && typeof x.id === 'number');
  } catch {
    return [];
  }
}

export const useHistoryStore = defineStore('history', () => {
  const items = ref<HistoryItem[]>([]);
  let nextId = 1;

  /** 当前选中的历史记录 id */
  const selectedId = ref<number | null>(null);

  /** 当前选中的历史记录对象 */
  const selectedItem = computed<HistoryItem | null>(
    () => items.value.find((i) => i.id === selectedId.value) ?? null,
  );

  /** 选中某条历史记录（不覆盖 Editor） */
  function selectItem(id: number | null) {
    selectedId.value = id;
  }

  function add(item: Omit<HistoryItem, 'id' | 'ts'>) {
    const newItem = { ...item, id: nextId++, ts: Date.now() };
    items.value.unshift(newItem);
    if (items.value.length > MAX_ITEMS) {
      items.value = items.value.slice(0, MAX_ITEMS);
    }
    // 自动选中新记录
    selectedId.value = newItem.id;
    persist();
  }

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, serialize(items.value));
    } catch { /* quota or unavailable */ }
  }

  function load() {
    try {
      items.value = deserialize(localStorage.getItem(STORAGE_KEY));
      nextId = items.value.length > 0
        ? Math.max(...items.value.map((i) => i.id)) + 1
        : 1;
    } catch { /* localStorage unavailable */ }
  }

  function clear() {
    items.value = [];
    nextId = 1;
    selectedId.value = null;
    try { localStorage.removeItem(STORAGE_KEY); } catch { /* */ }
  }

  return { items, selectedId, selectedItem, add, load, clear, persist, selectItem };
});
