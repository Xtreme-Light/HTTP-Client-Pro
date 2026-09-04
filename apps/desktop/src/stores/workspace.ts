import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { useRequestStore } from './request';

export interface WorkspaceRoot {
  id: string;
  name: string;
  path: string;
  /** 是否为本地导入的目录（软链接模式） */
  isLinked: boolean;
}

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface EditorTab {
  path: string;
  name: string;
  content: string;
  isDirty: boolean;
  type: 'file' | 'settings';
}

const STORAGE_KEY = 'http-client-pro:workspaces';
const CURRENT_FILE_KEY = 'http-client-pro:current-file';
const SESSION_KEY = 'http-client-pro:session';
const SETTINGS_TAB_PATH = '__settings__';

export const useWorkspaceStore = defineStore('workspace', () => {
  const roots = ref<WorkspaceRoot[]>([]);
  const tabs = ref<EditorTab[]>([]);
  const activeTabPath = ref<string | null>(null);

  /** 当前激活标签页的路径（向后兼容） */
  const currentFilePath = computed(() => activeTabPath.value);
  const currentFileName = computed(() => {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    return tab?.name ?? null;
  });
  const isDirty = computed(() => {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    return tab?.isDirty ?? false;
  });
  const canSave = computed(() => {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    return tab?.type === 'file';
  });
  const isSettingsActive = computed(() => {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    return tab?.type === 'settings';
  });

  function load() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          roots.value = parsed.filter((r) => r && typeof r.path === 'string');
        }
      }
    } catch { /* */ }
  }

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(roots.value));
    } catch { /* */ }
  }

  function addRoot(path: string, name?: string, isLinked = false) {
    const existing = roots.value.find((r) => r.path === path);
    if (existing) return existing;
    const root: WorkspaceRoot = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name: name || path.split('/').pop() || path,
      path,
      isLinked,
    };
    roots.value.push(root);
    persist();
    return root;
  }

  function removeRoot(id: string) {
    roots.value = roots.value.filter((r) => r.id !== id);
    persist();
  }

  /** 打开文件到标签页（若已存在则激活，否则新建标签） */
  function openFile(path: string, name: string, content: string) {
    const requestStore = useRequestStore();
    const existing = tabs.value.find((t) => t.path === path);
    if (existing) {
      activeTabPath.value = path;
      requestStore.setSource(existing.content);
      return;
    }
    tabs.value.push({ path, name, content, isDirty: false, type: 'file' });
    activeTabPath.value = path;
    requestStore.setSource(content);
    try { localStorage.setItem(CURRENT_FILE_KEY, path); } catch { /* */ }
  }

  /** 关闭标签页，返回新的激活标签（若无则 null） */
  function closeTab(path: string) {
    const requestStore = useRequestStore();
    const idx = tabs.value.findIndex((t) => t.path === path);
    if (idx === -1) return;
    tabs.value.splice(idx, 1);
    if (activeTabPath.value === path) {
      if (tabs.value.length > 0) {
        const newIdx = Math.min(idx, tabs.value.length - 1);
        activeTabPath.value = tabs.value[newIdx].path;
        requestStore.setSource(tabs.value[newIdx].content);
      } else {
        activeTabPath.value = null;
        requestStore.setSource('');
        try { localStorage.removeItem(CURRENT_FILE_KEY); } catch { /* */ }
      }
    }
  }

  /** 切换激活标签页 */
  function switchTab(path: string) {
    const requestStore = useRequestStore();
    const tab = tabs.value.find((t) => t.path === path);
    if (!tab) return;
    activeTabPath.value = path;
    requestStore.setSource(tab.content);
    try { localStorage.setItem(CURRENT_FILE_KEY, path); } catch { /* */ }
  }

  /** 更新当前激活标签页的内容 */
  function updateActiveContent(content: string) {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    if (tab) tab.content = content;
  }

  function markDirty() {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    if (tab) tab.isDirty = true;
  }

  function markClean() {
    const tab = tabs.value.find((t) => t.path === activeTabPath.value);
    if (tab) tab.isDirty = false;
  }

  /** 标记指定标签页为已保存 */
  function markTabClean(path: string) {
    const tab = tabs.value.find((t) => t.path === path);
    if (tab) tab.isDirty = false;
  }

  /** 获取指定标签页 */
  function getTab(path: string): EditorTab | undefined {
    return tabs.value.find((t) => t.path === path);
  }

  /** 打开设置标签页（若已存在则激活，不可重复打开） */
  function openSettings() {
    const requestStore = useRequestStore();
    const existing = tabs.value.find((t) => t.path === SETTINGS_TAB_PATH);
    if (existing) {
      activeTabPath.value = SETTINGS_TAB_PATH;
      return;
    }
    tabs.value.push({
      path: SETTINGS_TAB_PATH,
      name: '设置',
      content: '',
      isDirty: false,
      type: 'settings',
    });
    activeTabPath.value = SETTINGS_TAB_PATH;
  }

  /** 保存当前现场（已打开的标签页及其内容 / 激活标签）— 程序退出时调用 */
  function saveSession() {
    try {
      localStorage.setItem(SESSION_KEY, JSON.stringify({
        tabs: tabs.value,
        activeTabPath: activeTabPath.value,
      }));
    } catch { /* */ }
  }

  /**
   * 恢复上次退出现场 — 按保存内容直接恢复（不从磁盘重读）。
   * 返回是否恢复成功；失败（无会话 / 数据损坏）时返回 false，
   * 由调用方回退到默认文件初始化流程。
   */
  function restoreSession(): boolean {
    try {
      const raw = localStorage.getItem(SESSION_KEY);
      if (!raw) return false;
      const parsed = JSON.parse(raw) as { tabs?: unknown; activeTabPath?: unknown };
      if (!parsed || !Array.isArray(parsed.tabs)) return false;
      const restored = (parsed.tabs as EditorTab[]).filter(
        (t) => t && typeof t.path === 'string' && typeof t.content === 'string'
          && (t.type === 'file' || t.type === 'settings'),
      );
      if (restored.length === 0) return false;

      const requestStore = useRequestStore();
      tabs.value = restored;
      const active = typeof parsed.activeTabPath === 'string'
        && restored.some((t) => t.path === parsed.activeTabPath)
        ? parsed.activeTabPath
        : restored[0].path;
      activeTabPath.value = active;

      const activeTab = restored.find((t) => t.path === active)!;
      const editorSource = activeTab.type === 'file'
        ? activeTab.content
        : restored.find((t) => t.type === 'file')?.content ?? '';
      requestStore.setSource(editorSource);
      return true;
    } catch {
      return false;
    }
  }

  return {
    roots, tabs, activeTabPath,
    currentFilePath, currentFileName, isDirty, canSave, isSettingsActive,
    load, persist, addRoot, removeRoot,
    openFile, closeTab, switchTab, updateActiveContent,
    markDirty, markClean, markTabClean, getTab, openSettings,
    saveSession, restoreSession,
  };
});
