import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore, isUntitledPath } from './workspace';
import { useRequestStore } from './request';

/** 内存虚拟文件系统 — 模拟 Tauri 侧的文件读写 */
const vfs = vi.hoisted(() => ({ files: new Map<string, string>() }));

vi.mock('../lib/backend/fs', () => ({
  getFs: () => ({
    async listDir() { return []; },
    async readFile(path: string) {
      const c = vfs.files.get(path);
      if (c === undefined) throw new Error(`Failed to read file: ${path}`);
      return c;
    },
    async writeFile(path: string, content: string) { vfs.files.set(path, content); },
    async createFile(path: string) { vfs.files.set(path, ''); },
    async createDir() { /* noop */ },
    async renamePath() { /* noop */ },
    async deleteFile(path: string) { vfs.files.delete(path); },
    async getDefaultWorkspace() { return '/ws'; },
    async pickDirectory() { return null; },
    async pickFile() { return null; },
  }),
}));

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vfs.files.clear();
});

describe('workspace store openFile', () => {
  it('creates a tab and syncs the editor source', () => {
    const ws = useWorkspaceStore();
    ws.openFile('/ws/a.http', 'a.http', '### a');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.currentFilePath).toBe('/ws/a.http');
    expect(useRequestStore().source).toBe('### a');
  });

  it('adopts new content when re-opening a clean tab (save-as over an open file)', () => {
    const ws = useWorkspaceStore();
    ws.openFile('/ws/a.http', 'a.http', '### old');
    ws.openFile('/ws/a.http', 'a.http', '### new');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.getTab('/ws/a.http')?.content).toBe('### new');
    expect(useRequestStore().source).toBe('### new');
  });

  it('keeps unsaved edits when re-opening a dirty tab', () => {
    const ws = useWorkspaceStore();
    ws.openFile('/ws/a.http', 'a.http', '### old');
    ws.updateActiveContent('### editing');
    ws.markDirty();
    ws.openFile('/ws/a.http', 'a.http', '### from disk');
    expect(ws.getTab('/ws/a.http')?.content).toBe('### editing');
    expect(ws.isDirty).toBe(true);
  });
});

describe('workspace store path normalization (Windows separators)', () => {
  it('treats backslash and forward-slash paths as the same file (no duplicate tab)', () => {
    const ws = useWorkspaceStore();
    // 引导块拼接：反斜杠目录 + 正斜杠文件名
    ws.openFile('C:\\Users\\me\\.http-client-pro/requests.http', 'requests.http', '### a');
    // 工作区树条目：全反斜杠
    ws.openFile('C:\\Users\\me\\.http-client-pro\\requests.http', 'requests.http', '### a');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0].path).toBe('C:/Users/me/.http-client-pro/requests.http');
  });

  it('closeTab / switchTab / getTab match regardless of separator', () => {
    const ws = useWorkspaceStore();
    ws.openFile('C:\\ws\\a.http', 'a.http', '### a');
    expect(ws.getTab('C:/ws/a.http')).toBeDefined();
    expect(ws.getTab('C:\\ws\\a.http')).toBeDefined();
    ws.openFile('C:\\ws\\b.http', 'b.http', '### b');
    ws.switchTab('C:\\ws\\a.http');
    expect(ws.currentFilePath).toBe('C:/ws/a.http');
    ws.closeTab('C:\\ws\\b.http');
    expect(ws.tabs).toHaveLength(1);
  });

  it('markTabClean matches regardless of separator', () => {
    const ws = useWorkspaceStore();
    ws.openFile('C:\\ws\\a.http', 'a.http', '### a');
    ws.markDirty();
    expect(ws.isDirty).toBe(true);
    ws.markTabClean('C:/ws/a.http');
    expect(ws.getTab('C:\\ws\\a.http')?.isDirty).toBe(false);
  });
});

describe('workspace store legacy session (pre-normalization localStorage)', () => {
  const SESSION_KEY = 'http-client-pro:session';
  const ROOTS_KEY = 'http-client-pro:workspaces';
  // 旧版本引导块拼接出的混合分隔符路径 与 工作区树的全反斜杠路径
  const MIXED = 'C:\\Users\\me\\.http-client-pro/requests.http';
  const BACKSLASH = 'C:\\Users\\me\\.http-client-pro\\requests.http';
  const NORMALIZED = 'C:/Users/me/.http-client-pro/requests.http';

  function seedSession(tabs: unknown[], activeTabPath: string | null) {
    localStorage.setItem(SESSION_KEY, JSON.stringify({ tabs, activeTabPath }));
  }

  it('merges separator-only duplicates into a single closable tab', () => {
    seedSession([
      { path: MIXED, name: 'requests.http', content: '### a', isDirty: false, type: 'file' },
      { path: BACKSLASH, name: 'requests.http', content: '### a', isDirty: false, type: 'file' },
    ], BACKSLASH);

    const ws = useWorkspaceStore();
    expect(ws.restoreSession()).toBe(true);
    expect(ws.tabs.map((t) => t.path)).toEqual([NORMALIZED]);
    expect(ws.currentFilePath).toBe(NORMALIZED);
    expect(useRequestStore().source).toBe('### a');

    // 标签栏关闭按钮回传的就是 tab.path，必须能命中并移除
    ws.closeTab(ws.tabs[0].path);
    expect(ws.tabs).toHaveLength(0);
    expect(ws.currentFilePath).toBeNull();
  });

  it('keeps the dirty copy when merging duplicates', () => {
    seedSession([
      { path: MIXED, name: 'requests.http', content: '### from disk', isDirty: false, type: 'file' },
      { path: BACKSLASH, name: 'requests.http', content: '### unsaved', isDirty: true, type: 'file' },
    ], MIXED);

    const ws = useWorkspaceStore();
    expect(ws.restoreSession()).toBe(true);
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0].content).toBe('### unsaved');
    expect(ws.isDirty).toBe(true);
  });

  it('normalizes the restored active tab path and keeps other tabs in order', () => {
    seedSession([
      { path: 'C:\\ws\\a.http', name: 'a.http', content: '### a', isDirty: false, type: 'file' },
      { path: BACKSLASH, name: 'requests.http', content: '### b', isDirty: false, type: 'file' },
    ], BACKSLASH);

    const ws = useWorkspaceStore();
    expect(ws.restoreSession()).toBe(true);
    expect(ws.tabs.map((t) => t.path)).toEqual(['C:/ws/a.http', NORMALIZED]);
    expect(ws.currentFilePath).toBe(NORMALIZED);
    expect(useRequestStore().source).toBe('### b');
  });

  it('normalizes legacy workspace roots so Default is not added twice', () => {
    localStorage.setItem(ROOTS_KEY, JSON.stringify([
      { id: '1', name: 'Default', path: 'C:\\Users\\me\\.http-client-pro', isLinked: false },
    ]));

    const ws = useWorkspaceStore();
    ws.load();
    expect(ws.roots.map((r) => r.path)).toEqual(['C:/Users/me/.http-client-pro']);

    ws.addRoot('C:/Users/me/.http-client-pro', 'Default');
    expect(ws.roots).toHaveLength(1);
  });
});

describe('workspace store fs change notification', () => {
  it('notifyFsChange bumps fsRevision', () => {
    const ws = useWorkspaceStore();
    const before = ws.fsRevision;
    ws.notifyFsChange();
    expect(ws.fsRevision).toBe(before + 1);
  });
});

describe('workspace store untitled tabs', () => {
  it('isUntitledPath matches only untitled-N paths', () => {
    expect(isUntitledPath('untitled-1')).toBe(true);
    expect(isUntitledPath('untitled-12')).toBe(true);
    expect(isUntitledPath('/ws/untitled-1.http')).toBe(false);
    expect(isUntitledPath('untitled.http')).toBe(false);
  });

  it('createUntitledTab creates sequential non-conflicting tabs', () => {
    const ws = useWorkspaceStore();
    ws.createUntitledTab();
    expect(ws.tabs[0]).toMatchObject({ path: 'untitled-1', name: 'Untitled.http', content: '' });
    expect(ws.currentFilePath).toBe('untitled-1');

    ws.createUntitledTab();
    expect(ws.tabs[1]).toMatchObject({ path: 'untitled-2', name: 'Untitled-2.http' });

    // 关闭第一个后再新建，应复用 untitled-1 而不产生冲突
    ws.closeTab('untitled-1');
    ws.createUntitledTab();
    expect(ws.tabs.map((t) => t.path)).toEqual(['untitled-2', 'untitled-1']);
  });

  it('saveTab saves an untitled tab into the default workspace and converts it in place', async () => {
    const ws = useWorkspaceStore();
    ws.addRoot('/ws', 'Default');
    ws.createUntitledTab();
    ws.updateActiveContent('### hello');
    ws.markDirty();

    expect(await ws.saveTab()).toBe(true);
    expect(vfs.files.get('/ws/Untitled.http')).toBe('### hello');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0]).toMatchObject({ path: '/ws/Untitled.http', name: 'Untitled.http', isDirty: false });
    expect(ws.currentFilePath).toBe('/ws/Untitled.http');
  });

  it('saveTab picks a unique name when Untitled.http already exists on disk', async () => {
    const ws = useWorkspaceStore();
    ws.addRoot('/ws', 'Default');
    vfs.files.set('/ws/Untitled.http', '### existing');
    ws.createUntitledTab();
    ws.updateActiveContent('### new');

    expect(await ws.saveTab()).toBe(true);
    expect(vfs.files.get('/ws/Untitled.http')).toBe('### existing');
    expect(vfs.files.get('/ws/Untitled-2.http')).toBe('### new');
    expect(ws.tabs[0].path).toBe('/ws/Untitled-2.http');
  });

  it('saveTab falls back to the backend default workspace when no root is present', async () => {
    const ws = useWorkspaceStore();
    ws.createUntitledTab();
    ws.updateActiveContent('### fallback');

    expect(await ws.saveTab()).toBe(true);
    expect(vfs.files.get('/ws/Untitled.http')).toBe('### fallback');
  });

  it('saveTab writes a regular file tab and marks it clean', async () => {
    const ws = useWorkspaceStore();
    ws.openFile('/ws/a.http', 'a.http', '### old');
    ws.updateActiveContent('### edited');
    ws.markDirty();
    expect(ws.isDirty).toBe(true);

    expect(await ws.saveTab()).toBe(true);
    expect(vfs.files.get('/ws/a.http')).toBe('### edited');
    expect(ws.isDirty).toBe(false);
  });
});
