import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore } from '../stores/workspace';

/** 内存虚拟文件系统 — 模拟 Tauri 侧的 read_dir / read_file / write_file */
const vfs = vi.hoisted(() => ({ files: new Map<string, string>() }));

vi.mock('../lib/backend/fs', () => ({
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

import WorkspaceSidebar from './WorkspaceSidebar.vue';

/** 展开第一个（默认）根目录 */
async function expandDefaultRoot(wrapper: ReturnType<typeof mount>) {
  await wrapper.find('.root-node .toggle').trigger('click');
  await flushPromises();
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vfs.files.clear();
});

describe('WorkspaceSidebar tree freshness', () => {
  it('shows the default requests.http created by App bootstrap after mount', async () => {
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();

    // App.vue 的 onMounted 晚于子组件执行：此时才创建默认文件
    vfs.files.set('/ws/requests.http', '### hello');

    await expandDefaultRoot(wrapper);
    expect(wrapper.text()).toContain('requests.http');
  });

  it('refreshes an already expanded dir on fs change notification', async () => {
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    await expandDefaultRoot(wrapper);
    expect(wrapper.text()).toContain('(empty)');

    // 保存「另存为」出来的新文件后通知工作区树
    vfs.files.set('/ws/saved.http', '### saved');
    useWorkspaceStore().notifyFsChange();
    await flushPromises();

    expect(wrapper.text()).toContain('saved.http');
  });
});
