import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore } from '../stores/workspace';

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

import EditorTabBar from './EditorTabBar.vue';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vfs.files.clear();
});

describe('EditorTabBar double-click to create', () => {
  it('creates an Untitled.http tab when double-clicking the blank area', async () => {
    const wrapper = mount(EditorTabBar);
    const ws = useWorkspaceStore();

    await wrapper.find('.editor-tab-bar').trigger('dblclick');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0]).toMatchObject({ path: 'untitled-1', name: 'Untitled.http' });
  });

  it('does not create a tab when double-clicking on an existing tab', async () => {
    const ws = useWorkspaceStore();
    ws.openFile('/ws/a.http', 'a.http', '### a');
    const wrapper = mount(EditorTabBar);

    await wrapper.find('.editor-tab').trigger('dblclick');
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0].path).toBe('/ws/a.http');
  });
});

describe('EditorTabBar close dialog for untitled tabs', () => {
  it('saves a dirty untitled tab into the default workspace and closes it', async () => {
    const ws = useWorkspaceStore();
    ws.addRoot('/ws', 'Default');
    ws.createUntitledTab();
    ws.updateActiveContent('### hello');
    ws.markDirty();

    const wrapper = mount(EditorTabBar);
    await wrapper.find('.tab-close').trigger('click');
    expect(wrapper.find('.modal-overlay').exists()).toBe(true);

    await wrapper.find('.btn-save').trigger('click');
    await flushPromises();

    expect(vfs.files.get('/ws/Untitled.http')).toBe('### hello');
    // 未命名标签被转换为正式文件标签，不应残留也不应被重复关闭
    expect(ws.tabs).toHaveLength(1);
    expect(ws.tabs[0].path).toBe('/ws/Untitled.http');
    expect(wrapper.find('.modal-overlay').exists()).toBe(false);
  });
});
