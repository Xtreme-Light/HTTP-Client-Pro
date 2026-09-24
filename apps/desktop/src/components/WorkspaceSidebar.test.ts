import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { useWorkspaceStore } from '../stores/workspace';

/** 内存虚拟文件系统 — 模拟 Tauri 侧的 read_dir / read_file / write_file */
const vfs = vi.hoisted(() => ({
  files: new Map<string, string>(),
  /** 可选的时间戳元数据，用于验证按创建/修改时间排序 */
  meta: new Map<string, { createdAt?: number; modifiedAt?: number }>(),
}));

vi.mock('../lib/backend/fs', () => ({
  getFs: () => ({
    async listDir(path: string) {
      const items: {
        name: string; path: string; isDir: boolean; createdAt?: number; modifiedAt?: number;
      }[] = [];
      for (const p of vfs.files.keys()) {
        if (p.substring(0, p.lastIndexOf('/')) === path) {
          const meta = vfs.meta.get(p);
          items.push({
            name: p.slice(p.lastIndexOf('/') + 1),
            path: p,
            isDir: false,
            createdAt: meta?.createdAt,
            modifiedAt: meta?.modifiedAt,
          });
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

/** 当前渲染出来的文件顺序（不含根目录行） */
function renderedNames(wrapper: ReturnType<typeof mount>): string[] {
  return wrapper.findAll('.children .tree-node .name').map((n) => n.text());
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vfs.files.clear();
  vfs.meta.clear();
});

describe('WorkspaceSidebar tree freshness', () => {
  it('shows files written externally after expanding the root', async () => {
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();

    vfs.files.set('/ws/requests.http', '### hello');

    await expandDefaultRoot(wrapper);
    expect(wrapper.text()).toContain('requests.http');
  });

  it('toggles expand/collapse when clicking anywhere on the root row', async () => {
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    vfs.files.set('/ws/a.http', '### a');

    // 点击根目录行（非三角箭头）即可展开
    await wrapper.find('.root-node').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('a.http');
    expect(wrapper.find('.root-node .toggle').text()).toBe('▼');

    // 再次点击收起
    await wrapper.find('.root-node').trigger('click');
    await flushPromises();
    expect(wrapper.text()).not.toContain('a.http');
    expect(wrapper.find('.root-node .toggle').text()).toBe('▶');
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

describe('WorkspaceSidebar file sorting', () => {
  it('sorts by name descending by default', async () => {
    vfs.files.set('/ws/20260923.http', '');
    vfs.files.set('/ws/20260924xxx.http', '');
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    await expandDefaultRoot(wrapper);

    expect(renderedNames(wrapper)).toEqual(['20260924xxx.http', '20260923.http']);
  });

  it('switches to ascending when clicking the order button', async () => {
    vfs.files.set('/ws/20260923.http', '');
    vfs.files.set('/ws/20260924xxx.http', '');
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    await expandDefaultRoot(wrapper);

    await wrapper.find('.sort-order-btn').trigger('click');
    await flushPromises();

    expect(renderedNames(wrapper)).toEqual(['20260923.http', '20260924xxx.http']);
    expect(wrapper.find('.sort-order-btn').text()).toBe('↑');
  });

  it('sorts by modified time when that field is selected', async () => {
    vfs.files.set('/ws/new-name.http', '');
    vfs.files.set('/ws/aaa.http', '');
    vfs.meta.set('/ws/new-name.http', { modifiedAt: 1000 });
    vfs.meta.set('/ws/aaa.http', { modifiedAt: 2000 });
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    await expandDefaultRoot(wrapper);
    expect(renderedNames(wrapper)).toEqual(['new-name.http', 'aaa.http']);

    await wrapper.find('.sort-select').setValue('modified');
    await flushPromises();

    expect(renderedNames(wrapper)).toEqual(['aaa.http', 'new-name.http']);
  });

  it('sorts by created time when that field is selected', async () => {
    vfs.files.set('/ws/new-name.http', '');
    vfs.files.set('/ws/aaa.http', '');
    vfs.meta.set('/ws/new-name.http', { createdAt: 3000, modifiedAt: 1000 });
    vfs.meta.set('/ws/aaa.http', { createdAt: 1000, modifiedAt: 3000 });
    const wrapper = mount(WorkspaceSidebar);
    await flushPromises();
    await expandDefaultRoot(wrapper);

    await wrapper.find('.sort-select').setValue('created');
    await flushPromises();

    expect(renderedNames(wrapper)).toEqual(['new-name.http', 'aaa.http']);
  });

  it('remembers the sort preference across remounts', async () => {
    vfs.files.set('/ws/a.http', '');
    const first = mount(WorkspaceSidebar);
    await flushPromises();
    await first.find('.sort-order-btn').trigger('click');

    const second = mount(WorkspaceSidebar);
    await flushPromises();
    expect(second.find('.sort-order-btn').text()).toBe('↑');
  });
});
