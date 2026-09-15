import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore } from './workspace';
import { useRequestStore } from './request';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
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
