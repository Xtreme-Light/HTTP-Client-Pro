import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore } from './stores/workspace';
import { useRequestStore } from './stores/request';

/** 内存虚拟文件系统 — 模拟 Tauri 侧的文件命令 */
const vfs = vi.hoisted(() => ({ files: new Map<string, string>(), calls: [] as string[] }));

vi.mock('./lib/backend/fs', () => ({
  getFs: () => ({
    async listDir(path: string) {
      const items: { name: string; path: string; isDir: boolean }[] = [];
      for (const p of vfs.files.keys()) {
        if (p.substring(0, p.lastIndexOf('/')) === path) {
          items.push({ name: p.slice(p.lastIndexOf('/') + 1), path: p, isDir: false });
        }
      }
      return items;
    },
    async readFile(path: string) {
      const c = vfs.files.get(path);
      if (c === undefined) throw new Error(`Failed to read file: ${path}`);
      return c;
    },
    async writeFile(path: string, content: string) {
      vfs.calls.push(`write:${path}`);
      vfs.files.set(path, content);
    },
    async createFile(path: string) { vfs.files.set(path, ''); },
    async createDir() { /* noop */ },
    async renamePath() { /* noop */ },
    async deleteFile(path: string) { vfs.files.delete(path); },
    async getDefaultWorkspace() { return '/ws'; },
    async pickDirectory() { return null; },
    async pickFile() { return null; },
  }),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ setZoom: async () => { /* noop */ } }),
}));

/** 记录 Tauri 关窗钩子与销毁调用，用于验证退出前落盘 */
const win = vi.hoisted(() => ({
  closeHandlers: [] as ((event: { preventDefault: () => void }) => Promise<void>)[],
  destroyed: 0,
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: (event: { preventDefault: () => void }) => Promise<void>) => {
      win.closeHandlers.push(handler);
      return () => { /* unlisten */ };
    },
    destroy: async () => { win.destroyed++; },
  }),
}));

vi.mock('./composables/useRunCurrent', () => ({
  detectAdapter: () => ({}),
  setBackendAdapter: () => { /* noop */ },
  useRunCurrent: () => ({ run: async () => { /* noop */ } }),
}));

vi.mock('./lib/backend', () => ({ detectAdapter: () => ({}) }));

import App from './App.vue';

/** 编辑器等重型子组件与启动逻辑无关，直接桩掉 */
const STUBS = {
  TitleBar: true,
  EditorPane: true,
  EditorTabBar: true,
  SettingsPane: true,
  ResponsePane: true,
  HistoryPanel: true,
  Splitpanes: { template: '<div><slot /></div>' },
  Pane: { template: '<div><slot /></div>' },
};

function mountApp() {
  return mount(App, { global: { stubs: STUBS } });
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vfs.files.clear();
  vfs.calls.length = 0;
  win.closeHandlers.length = 0;
  win.destroyed = 0;
});

describe('first launch bootstrap', () => {
  it('creates the default requests.http and shows it in the workspace tree', async () => {
    const wrapper = mountApp();
    await flushPromises();

    const ws = useWorkspaceStore();
    expect(ws.tabs.map((t) => t.name)).toEqual(['requests.http']);
    expect(vfs.files.has('/ws/requests.http')).toBe(true);

    // 展开 Default 根目录 → 必须能看到启动时创建的默认文件
    await wrapper.find('.root-node .toggle').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('requests.http');
  });

  it('opens an existing default file without rewriting it', async () => {
    vfs.files.set('/ws/requests.http', '### from disk');
    const wrapper = mountApp();
    await flushPromises();

    expect(vfs.calls).not.toContain('write:/ws/requests.http');
    expect(useRequestStore().source).toBe('### from disk');
    wrapper.unmount();
  });

  it('saves the latest tab content, not the debounced editor store', async () => {
    mountApp();
    await flushPromises();

    const ws = useWorkspaceStore();
    // 模拟编辑器输入：标签页内容同步更新，requestStore 仍在防抖窗口内
    ws.updateActiveContent('### edited');
    ws.markDirty();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true }));
    await flushPromises();

    expect(vfs.files.get('/ws/requests.http')).toBe('### edited');
    expect(ws.isDirty).toBe(false);
  });
});

describe('session persistence', () => {
  const SESSION_KEY = 'http-client-pro:session';

  afterEach(() => {
    vi.useRealTimers();
  });

  function readSession(): { tabs: { path: string; content: string; isDirty: boolean }[]; activeTabPath: string } {
    const raw = localStorage.getItem(SESSION_KEY);
    expect(raw, 'session should be written to localStorage').toBeTruthy();
    return JSON.parse(raw!);
  }

  it('autosaves the session without any unload event', async () => {
    vi.useFakeTimers();
    const wrapper = mountApp();
    await vi.advanceTimersByTimeAsync(400);

    const session = readSession();
    expect(session.activeTabPath).toBe('/ws/requests.http');
    expect(session.tabs.map((t) => t.path)).toEqual(['/ws/requests.http']);
    wrapper.unmount();
  });

  it('flushes the session when Tauri requests the window to close', async () => {
    const wrapper = mountApp();
    await flushPromises();

    const ws = useWorkspaceStore();
    ws.updateActiveContent('### latest');
    ws.markDirty();

    // 关窗请求：必须先落盘现场，再销毁窗口（beforeunload 在 Tauri 中不可靠）
    const event = { preventDefault: vi.fn() };
    await win.closeHandlers.at(-1)!(event);

    expect(event.preventDefault).toHaveBeenCalled();
    expect(win.destroyed).toBe(1);
    const session = readSession();
    expect(session.tabs[0].content).toBe('### latest');
    expect(session.tabs[0].isDirty).toBe(true);
    wrapper.unmount();
  });

  it('restores the previous session instead of bootstrapping the default file', async () => {
    vfs.files.set('/ws/a.http', '### from disk');
    localStorage.setItem(SESSION_KEY, JSON.stringify({
      tabs: [
        { path: '/ws/a.http', name: 'a.http', content: '### stale', isDirty: false, type: 'file' },
        { path: '/ws/b.http', name: 'b.http', content: '### unsaved', isDirty: true, type: 'file' },
      ],
      activeTabPath: '/ws/a.http',
    }));

    const wrapper = mountApp();
    await flushPromises();

    const ws = useWorkspaceStore();
    expect(ws.tabs.map((t) => t.path)).toEqual(['/ws/a.http', '/ws/b.http']);
    // 干净标签采用磁盘上的最新内容，脏标签保留未保存的编辑
    expect(ws.getTab('/ws/a.http')!.content).toBe('### from disk');
    expect(ws.getTab('/ws/b.http')!.content).toBe('### unsaved');
    expect(useRequestStore().source).toBe('### from disk');
    expect(vfs.calls).not.toContain('write:/ws/requests.http');
    wrapper.unmount();
  });
});
