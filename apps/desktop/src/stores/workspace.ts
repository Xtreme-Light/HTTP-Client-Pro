import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { useRequestStore } from './request';
import { normalizePath } from '../lib/path';
import { getExample } from '../lib/examples';
import { getFs } from '../lib/backend/fs';

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
  type: 'file' | 'settings' | 'example';
}

const STORAGE_KEY = 'http-client-pro:workspaces';
const CURRENT_FILE_KEY = 'http-client-pro:current-file';
const SESSION_KEY = 'http-client-pro:session';
const SETTINGS_TAB_PATH = '__settings__';
/** 未保存的临时标签页路径格式 */
const UNTITLED_RE = /^untitled-\d+$/;

/** 是否为未命名临时标签页路径 */
export function isUntitledPath(path: string): boolean {
  return UNTITLED_RE.test(path);
}

export const useWorkspaceStore = defineStore('workspace', () => {
  const roots = ref<WorkspaceRoot[]>([]);
  const tabs = ref<EditorTab[]>([]);
  const activeTabPath = ref<string | null>(null);
  /** 文件系统变更计数 — 工作区树监听它来失效目录缓存 */
  const fsRevision = ref(0);

  /**
   * 按路径定位标签页下标 — 入参与已存路径两侧都做分隔符规范化。
   * 只规范化一侧会让历史会话里遗留的反斜杠路径永远匹配不上，
   * 表现为标签页点关闭没有任何反应。
   */
  function indexOfTab(path: string | null): number {
    if (path === null) return -1;
    const key = normalizePath(path);
    return tabs.value.findIndex((t) => normalizePath(t.path) === key);
  }

  /** 当前激活的标签页 */
  const activeTab = computed(() => {
    const idx = indexOfTab(activeTabPath.value);
    return idx === -1 ? undefined : tabs.value[idx];
  });

  /** 当前激活标签页的路径（向后兼容） */
  const currentFilePath = computed(() => activeTabPath.value);
  const currentFileName = computed(() => activeTab.value?.name ?? null);
  const isDirty = computed(() => activeTab.value?.isDirty ?? false);
  const canSave = computed(() => activeTab.value?.type === 'file');
  const isSettingsActive = computed(() => activeTab.value?.type === 'settings');

  function load() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          // 历史数据可能存有反斜杠路径 — 规范化后去重，避免侧栏出现两个同名根目录
          const normalized = parsed
            .filter((r) => r && typeof r.path === 'string')
            .map((r) => ({ ...r, path: normalizePath(r.path) }));
          const seen = new Set<string>();
          roots.value = normalized.filter((r) => {
            if (seen.has(r.path)) return false;
            seen.add(r.path);
            return true;
          });
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
    const key = normalizePath(path);
    const existing = roots.value.find((r) => normalizePath(r.path) === key);
    if (existing) return existing;
    const root: WorkspaceRoot = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name: name || key.split('/').pop() || key,
      path: key,
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

  /** 通知文件系统已变化（新建 / 保存 / 删除），驱动工作区树刷新 */
  function notifyFsChange() {
    fsRevision.value++;
  }

  /** 打开文件到标签页（若已存在则激活，否则新建标签） */
  function openFile(path: string, name: string, content: string) {
    const requestStore = useRequestStore();
    const key = normalizePath(path);
    const existingIdx = indexOfTab(key);
    if (existingIdx !== -1) {
      const existing = tabs.value[existingIdx];
      activeTabPath.value = key;
      // 无未保存修改时采用传入的最新内容（如「另存为」覆盖已打开的文件），
      // 有未保存修改时保留编辑中的内容，避免被覆盖丢失。
      if (!existing.isDirty) existing.content = content;
      requestStore.setSource(existing.content);
      return;
    }
    tabs.value.push({ path: key, name, content, isDirty: false, type: 'file' });
    activeTabPath.value = key;
    requestStore.setSource(content);
    try { localStorage.setItem(CURRENT_FILE_KEY, key); } catch { /* */ }
  }

  /** 关闭标签页，返回新的激活标签（若无则 null） */
  function closeTab(path: string) {
    const requestStore = useRequestStore();
    const key = normalizePath(path);
    const idx = indexOfTab(key);
    if (idx === -1) return;
    tabs.value.splice(idx, 1);
    if (activeTabPath.value === key) {
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
    const key = normalizePath(path);
    const tab = tabs.value[indexOfTab(key)];
    if (!tab) return;
    activeTabPath.value = key;
    requestStore.setSource(tab.content);
    try { localStorage.setItem(CURRENT_FILE_KEY, key); } catch { /* */ }
  }

  /** 更新当前激活标签页的内容 */
  function updateActiveContent(content: string) {
    const tab = activeTab.value;
    if (tab) tab.content = content;
  }

  function markDirty() {
    const tab = activeTab.value;
    if (tab) tab.isDirty = true;
  }

  function markClean() {
    const tab = activeTab.value;
    if (tab) tab.isDirty = false;
  }

  /** 标记指定标签页为已保存 */
  function markTabClean(path: string) {
    const tab = tabs.value[indexOfTab(path)];
    if (tab) tab.isDirty = false;
  }

  /** 获取指定标签页 */
  function getTab(path: string): EditorTab | undefined {
    return tabs.value[indexOfTab(path)];
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

  /** 打开内置请求示例标签页（只读，若已存在则激活，不参与会话持久化） */
  function openExample(id: string) {
    const example = getExample(id);
    if (!example) return;
    const requestStore = useRequestStore();
    const path = `__example_${id}__`;
    const existing = tabs.value.find((t) => t.path === path);
    if (existing) {
      activeTabPath.value = path;
      requestStore.setSource(existing.content);
      return;
    }
    tabs.value.push({
      path,
      name: example.title,
      content: example.content,
      isDirty: false,
      type: 'example',
    });
    activeTabPath.value = path;
    requestStore.setSource(example.content);
  }

  /** 新建未命名标签页（Untitled.http）— 保存时写入默认工作区目录 */
  function createUntitledTab() {
    let n = 1;
    while (indexOfTab(`untitled-${n}`) !== -1) n++;
    const path = `untitled-${n}`;
    openFile(path, n === 1 ? 'Untitled.http' : `Untitled-${n}.http`, '');
  }

  /** 默认工作区目录：优先取非软链接的根（Default），否则第一个根，最后回退到后端默认路径 */
  async function defaultWorkspaceDir(): Promise<string> {
    const root = roots.value.find((r) => !r.isLinked) ?? roots.value[0];
    if (root) return root.path;
    const fs = getFs();
    if (!fs) throw new Error('No workspace available');
    return fs.getDefaultWorkspace();
  }

  /** 在目录下生成不冲突的 Untitled 文件名（Untitled.http / Untitled-2.http …） */
  async function uniqueUntitledName(dir: string, name: string): Promise<string> {
    const fs = getFs()!;
    const fileName = /\.[^./\\]+$/.test(name) ? name : `${name}.http`;
    const dot = fileName.lastIndexOf('.');
    const base = fileName.slice(0, dot);
    const ext = fileName.slice(dot);
    const exists = async (p: string) => {
      try { await fs.readFile(p); return true; } catch { return false; }
    };
    let candidate = fileName;
    let n = 2;
    while (await exists(`${dir}/${candidate}`)) {
      candidate = `${base}-${n}${ext}`;
      n++;
    }
    return candidate;
  }

  /**
   * 保存标签页（缺省为当前激活标签）到磁盘，失败时抛出异常由调用方提示。
   * 未命名标签页保存到默认工作区目录，并原地转为正式文件标签页。
   * 返回是否执行了保存（非文件标签页 / 无后端时返回 false）。
   */
  async function saveTab(path?: string | null): Promise<boolean> {
    const fs = getFs();
    if (!fs) return false;
    const tab = getTab(path ?? activeTabPath.value ?? '');
    if (!tab || tab.type !== 'file') return false;

    if (isUntitledPath(tab.path)) {
      const dir = await defaultWorkspaceDir();
      const fileName = await uniqueUntitledName(dir, tab.name);
      const fullPath = `${dir}/${fileName}`;
      await fs.writeFile(fullPath, tab.content);
      // 先打开新文件标签再关闭临时标签，保持激活焦点不跳动
      openFile(fullPath, fileName, tab.content);
      closeTab(tab.path);
      markTabClean(fullPath);
      notifyFsChange();
      return true;
    }

    // 取标签页的最新内容（编辑器回写 requestStore 有防抖延迟）
    await fs.writeFile(tab.path, tab.content);
    markTabClean(tab.path);
    notifyFsChange();
    return true;
  }

  /** 保存当前现场（已打开的标签页及其内容 / 激活标签）— 程序退出时调用 */
  function saveSession() {
    try {
      localStorage.setItem(SESSION_KEY, JSON.stringify({
        // 只读示例标签页不持久化（重开成本为零，且避免过期内容残留）
        tabs: tabs.value.filter((t) => t.type !== 'example'),
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

      // 旧版本写入的会话可能带有反斜杠/混合分隔符路径，同一文件因此变成两个
      // 关不掉的标签页。这里统一规范化并按路径合并；有未保存修改的副本优先，
      // 避免合并时丢失编辑内容。
      const byPath = new Map<string, EditorTab>();
      for (const t of restored) {
        const key = normalizePath(t.path);
        const kept = byPath.get(key);
        if (!kept || (t.isDirty && !kept.isDirty)) byPath.set(key, { ...t, path: key });
      }
      const merged = [...byPath.values()];

      const requestStore = useRequestStore();
      tabs.value = merged;
      const requested = typeof parsed.activeTabPath === 'string'
        ? normalizePath(parsed.activeTabPath)
        : null;
      const active = requested && byPath.has(requested) ? requested : merged[0].path;
      activeTabPath.value = active;

      const current = byPath.get(active)!;
      const editorSource = current.type === 'file'
        ? current.content
        : merged.find((t) => t.type === 'file')?.content ?? '';
      requestStore.setSource(editorSource);
      return true;
    } catch {
      return false;
    }
  }

  return {
    roots, tabs, activeTabPath, fsRevision, activeTab,
    currentFilePath, currentFileName, isDirty, canSave, isSettingsActive,
    load, persist, addRoot, removeRoot, notifyFsChange,
    openFile, closeTab, switchTab, updateActiveContent,
    markDirty, markClean, markTabClean, getTab, openSettings, openExample,
    createUntitledTab, saveTab,
    saveSession, restoreSession,
  };
});
