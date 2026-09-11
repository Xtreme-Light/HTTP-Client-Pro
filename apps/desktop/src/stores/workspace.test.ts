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

describe('workspace store fs change notification', () => {
  it('notifyFsChange bumps fsRevision', () => {
    const ws = useWorkspaceStore();
    const before = ws.fsRevision;
    ws.notifyFsChange();
    expect(ws.fsRevision).toBe(before + 1);
  });
});
