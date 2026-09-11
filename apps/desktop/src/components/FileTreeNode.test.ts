import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import FileTreeNode from './FileTreeNode.vue';
import type { FileEntry } from '../stores/workspace';

/** 两层目录结构：docs/sub/deep.http + docs/note.http */
const dirCache = new Map<string, FileEntry[]>([
  ['/ws/docs', [
    { name: 'sub', path: '/ws/docs/sub', isDir: true },
    { name: 'note.http', path: '/ws/docs/note.http', isDir: false },
  ]],
  ['/ws/docs/sub', [
    { name: 'deep.http', path: '/ws/docs/sub/deep.http', isDir: false },
  ]],
]);

function mountTree(expanded: string[]) {
  return mount(FileTreeNode, {
    props: {
      entry: { name: 'docs', path: '/ws/docs', isDir: true },
      expandedDirs: new Set(expanded),
      dirCache,
    },
  });
}

describe('FileTreeNode nested events', () => {
  it('forwards file-click from a nested file', async () => {
    const wrapper = mountTree(['/ws/docs']);
    const nodes = wrapper.findAll('.tree-node');
    expect(nodes).toHaveLength(3); // docs / sub / note.http

    await nodes[2].trigger('click');
    const emitted = wrapper.emitted('file-click');
    expect(emitted).toHaveLength(1);
    expect((emitted![0][0] as FileEntry).path).toBe('/ws/docs/note.http');
  });

  it('forwards file-click from a nested directory so it can be expanded', async () => {
    const wrapper = mountTree(['/ws/docs']);
    await wrapper.findAll('.tree-node')[1].trigger('click');

    const emitted = wrapper.emitted('file-click');
    expect(emitted).toHaveLength(1);
    const entry = emitted![0][0] as FileEntry;
    expect(entry.path).toBe('/ws/docs/sub');
    expect(entry.isDir).toBe(true);
  });

  it('renders and forwards clicks from grand-children', async () => {
    const wrapper = mountTree(['/ws/docs', '/ws/docs/sub']);
    const nodes = wrapper.findAll('.tree-node');
    expect(nodes).toHaveLength(4); // docs / sub / deep.http / note.http
    expect(wrapper.text()).toContain('deep.http');

    await nodes[2].trigger('click');
    const emitted = wrapper.emitted('file-click');
    expect((emitted![0][0] as FileEntry).path).toBe('/ws/docs/sub/deep.http');
  });

  it('forwards context-menu from a nested node', async () => {
    const wrapper = mountTree(['/ws/docs']);
    await wrapper.findAll('.tree-node')[2].trigger('contextmenu');

    const emitted = wrapper.emitted('context-menu');
    expect(emitted).toHaveLength(1);
    expect((emitted![0][0] as FileEntry).path).toBe('/ws/docs/note.http');
    expect(emitted![0][1]).toBeInstanceOf(Event);
  });
});
