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
    ws.openFile('C:\\ws\\b.http', 'b.http', '### b');
    ws.switchTab('C:\\ws\\a.http');
    expect(ws.currentFilePath).toBe('C:/ws/a.http');
    ws.closeTab('C:\\ws\\b.http');
    expect(ws.tabs).toHaveLength(1);
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
