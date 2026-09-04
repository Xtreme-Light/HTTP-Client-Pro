<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useWorkspaceStore, type FileEntry } from '../stores/workspace';
import { getFs } from '../lib/backend/fs';
import FileTreeNode from './FileTreeNode.vue';

const workspaceStore = useWorkspaceStore();
const fs = getFs();

/** 展开的目录路径集合 */
const expandedDirs = ref<Set<string>>(new Set());
/** 条目右键菜单状态 */
const contextMenu = ref<{ x: number; y: number; path: string; name: string; isDir: boolean } | null>(null);
/** 空白区域右键菜单状态 */
const blankMenu = ref<{ x: number; y: number; rootPath: string | null } | null>(null);
/** 目录树容器（空白区域右键时定位所属根目录） */
const treeEl = ref<HTMLElement>();
/** 目录内容缓存 */
const dirCache = ref<Map<string, FileEntry[]>>(new Map());
/** 新建文件对话框 */
const showNewFile = ref(false);
const newFileParent = ref('');
const newFileName = ref('');
/** 新建文件夹对话框 */
const showNewDir = ref(false);
const newDirParent = ref('');
const newDirName = ref('');
/** 重命名对话框 */
const showRename = ref(false);
const renameOldPath = ref('');
const renameNewName = ref('');

const roots = computed(() => workspaceStore.roots);

async function loadDir(path: string): Promise<FileEntry[]> {
  if (!fs) return [];
  if (dirCache.value.has(path)) {
    return dirCache.value.get(path)!;
  }
  try {
    const items = await fs.listDir(path);
    dirCache.value.set(path, items);
    return items;
  } catch (e) {
    console.error('Failed to load dir:', e);
    return [];
  }
}

async function toggleDir(path: string) {
  const newSet = new Set(expandedDirs.value);
  if (newSet.has(path)) {
    newSet.delete(path);
  } else {
    newSet.add(path);
    await loadDir(path);
  }
  expandedDirs.value = newSet;
}

async function refreshDir(path: string) {
  dirCache.value.delete(path);
  if (expandedDirs.value.has(path)) {
    await loadDir(path);
  }
}

async function onFileClick(entry: FileEntry) {
  if (entry.isDir) {
    await toggleDir(entry.path);
  } else {
    await openFile(entry);
  }
}

function onContextMenu(e: MouseEvent, entry: FileEntry) {
  e.preventDefault();
  e.stopPropagation();
  contextMenu.value = {
    x: e.clientX,
    y: e.clientY,
    path: entry.path,
    name: entry.name,
    isDir: entry.isDir,
  };
}

/** 文件树条目右键（来自 FileTreeNode 的 emit） */
function onEntryContextMenu(entry: FileEntry, e: MouseEvent) {
  contextMenu.value = {
    x: e.clientX,
    y: e.clientY,
    path: entry.path,
    name: entry.name,
    isDir: entry.isDir,
  };
}

/** 空白区域右键 — 根据点击位置定位所属根目录，否则取第一个根目录 */
function onBlankContextMenu(e: MouseEvent) {
  e.preventDefault();
  let rootPath: string | null = null;
  if (treeEl.value && workspaceStore.roots.length > 0) {
    const nodes = treeEl.value.querySelectorAll<HTMLElement>('[data-root-path]');
    for (const node of nodes) {
      const rect = node.getBoundingClientRect();
      if (e.clientY >= rect.top && e.clientY <= rect.bottom) {
        rootPath = node.dataset.rootPath ?? null;
        break;
      }
    }
  }
  rootPath ??= workspaceStore.roots[0]?.path ?? null;
  blankMenu.value = { x: e.clientX, y: e.clientY, rootPath };
}

async function openFile(entry: FileEntry) {
  if (!fs) return;
  try {
    const content = await fs.readFile(entry.path);
    workspaceStore.openFile(entry.path, entry.name, content);
  } catch (e) {
    console.error('Failed to read file:', e);
    alert(`Failed to read file: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function importDirectory() {
  if (!fs) return;
  try {
    const dir = await fs.pickDirectory();
    if (dir) {
      workspaceStore.addRoot(dir, undefined, true);
      await toggleDir(dir);
    }
  } catch (e) {
    console.error('Failed to import directory:', e);
  }
}

function closeContextMenu() {
  contextMenu.value = null;
  blankMenu.value = null;
}

/** 新建文件的父目录：目录本身 / 文件所在目录 */
function parentDirOf(path: string, isDir: boolean): string {
  return isDir ? path : path.substring(0, path.lastIndexOf('/'));
}

function startNewFile(path: string, isDir: boolean) {
  newFileParent.value = parentDirOf(path, isDir);
  newFileName.value = '';
  showNewFile.value = true;
  closeContextMenu();
}

async function confirmNewFile() {
  if (!fs || !newFileName.value.trim()) return;
  const name = newFileName.value.trim();
  const fileName = name.endsWith('.http') || name.includes('.') ? name : `${name}.http`;
  const fullPath = `${newFileParent.value}/${fileName}`;
  try {
    await fs.createFile(fullPath);
    await refreshDir(newFileParent.value);
    showNewFile.value = false;
  } catch (e) {
    alert(`Failed to create file: ${e instanceof Error ? e.message : String(e)}`);
  }
}

function startNewDir(path: string, isDir: boolean) {
  newDirParent.value = parentDirOf(path, isDir);
  newDirName.value = '';
  showNewDir.value = true;
  closeContextMenu();
}

async function confirmNewDir() {
  if (!fs || !newDirName.value.trim()) return;
  const dirName = newDirName.value.trim();
  const fullPath = `${newDirParent.value}/${dirName}`;
  try {
    await fs.createDir(fullPath);
    await refreshDir(newDirParent.value);
    showNewDir.value = false;
  } catch (e) {
    alert(`Failed to create directory: ${e instanceof Error ? e.message : String(e)}`);
  }
}

function startRename(path: string, name: string) {
  renameOldPath.value = path;
  renameNewName.value = name;
  showRename.value = true;
  closeContextMenu();
}

async function confirmRename() {
  if (!fs || !renameNewName.value.trim()) return;
  const newName = renameNewName.value.trim();
  const parentPath = renameOldPath.value.substring(0, renameOldPath.value.lastIndexOf('/'));
  const newPath = `${parentPath}/${newName}`;
  try {
    await fs.renamePath(renameOldPath.value, newPath);
    for (const [dirPath] of dirCache.value) {
      if (dirPath === parentPath) {
        dirCache.value.delete(dirPath);
        if (expandedDirs.value.has(dirPath)) {
          await loadDir(dirPath);
        }
      }
    }
    showRename.value = false;
  } catch (e) {
    alert(`Failed to rename: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function confirmDelete(path: string, isDir: boolean) {
  if (!fs) return;
  const msg = isDir ? '确定删除该目录及其全部内容？' : '确定删除该文件？';
  if (!confirm(msg)) {
    closeContextMenu();
    return;
  }
  try {
    await fs.deleteFile(path);
    const parentPath = path.substring(0, path.lastIndexOf('/'));
    await refreshDir(parentPath);
    closeContextMenu();
  } catch (e) {
    alert(`Failed to delete: ${e instanceof Error ? e.message : String(e)}`);
    closeContextMenu();
  }
}

function removeRoot(id: string) {
  workspaceStore.removeRoot(id);
}

function getDirEntries(path: string): FileEntry[] {
  return dirCache.value.get(path) ?? [];
}

onMounted(async () => {
  workspaceStore.load();
  if (fs && workspaceStore.roots.length === 0) {
    try {
      const defaultPath = await fs.getDefaultWorkspace();
      workspaceStore.addRoot(defaultPath, 'Default');
    } catch (e) {
      console.error('Failed to init default workspace:', e);
    }
  }
  // 加载所有根目录的内容到 dirCache
  if (fs) {
    for (const root of workspaceStore.roots) {
      await loadDir(root.path);
    }
  }
});
</script>

<template>
  <div class="workspace-sidebar" @click="closeContextMenu" @contextmenu="onBlankContextMenu">
    <div class="sidebar-header">
      <span class="sidebar-title">Workspace</span>
      <button class="icon-btn" title="Import directory" @click="importDirectory">+</button>
    </div>

    <div ref="treeEl" class="tree-container">
      <div v-for="root in roots" :key="root.id" class="root-item" :data-root-path="root.path">
        <div
          class="tree-node root-node"
          @contextmenu="onContextMenu($event, { name: root.name, path: root.path, isDir: true })"
        >
          <span class="toggle" @click="toggleDir(root.path)">
            {{ expandedDirs.has(root.path) ? '▼' : '▶' }}
          </span>
          <span class="file-icon folder-icon">📁</span>
          <span class="name root-name">{{ root.name }}</span>
          <button class="remove-root-btn" title="Remove from workspace" @click.stop="removeRoot(root.id)">×</button>
        </div>

        <div v-if="expandedDirs.has(root.path)" class="children">
          <FileTreeNode
            v-for="entry in getDirEntries(root.path)"
            :key="entry.path"
            :entry="entry"
            :expanded-dirs="expandedDirs"
            :dir-cache="dirCache"
            @toggle-dir="toggleDir"
            @file-click="onFileClick"
            @context-menu="onEntryContextMenu"
          />
          <div v-if="getDirEntries(root.path).length === 0" class="empty-dir">
            (empty)
          </div>
        </div>
      </div>
    </div>

    <!-- 条目右键菜单 -->
    <div
      v-if="contextMenu"
      class="context-menu"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
      @click.stop
    >
      <button class="menu-item" @click="startNewFile(contextMenu.path, contextMenu.isDir)">新建文件</button>
      <button class="menu-item" @click="startNewDir(contextMenu.path, contextMenu.isDir)">新建文件夹</button>
      <button class="menu-item" @click="startRename(contextMenu.path, contextMenu.name)">重命名</button>
      <button class="menu-item danger" @click="confirmDelete(contextMenu.path, contextMenu.isDir)">删除</button>
    </div>

    <!-- 空白区域右键菜单 -->
    <div
      v-if="blankMenu"
      class="context-menu"
      :style="{ left: blankMenu.x + 'px', top: blankMenu.y + 'px' }"
      @click.stop
    >
      <button v-if="blankMenu.rootPath" class="menu-item" @click="startNewFile(blankMenu.rootPath, true)">新建文件</button>
      <button v-if="blankMenu.rootPath" class="menu-item" @click="startNewDir(blankMenu.rootPath, true)">新建文件夹</button>
      <button class="menu-item" @click="importDirectory(); closeContextMenu()">引入文件夹</button>
    </div>

    <!-- 新建文件对话框 -->
    <div v-if="showNewFile" class="modal-overlay" @click="showNewFile = false">
      <div class="modal" @click.stop>
        <h3>新建文件</h3>
        <input v-model="newFileName" placeholder="filename.http" @keyup.enter="confirmNewFile" autofocus />
        <div class="modal-actions">
          <button @click="showNewFile = false">取消</button>
          <button class="primary" @click="confirmNewFile">创建</button>
        </div>
      </div>
    </div>

    <!-- 新建文件夹对话框 -->
    <div v-if="showNewDir" class="modal-overlay" @click="showNewDir = false">
      <div class="modal" @click.stop>
        <h3>新建文件夹</h3>
        <input v-model="newDirName" placeholder="folder-name" @keyup.enter="confirmNewDir" autofocus />
        <div class="modal-actions">
          <button @click="showNewDir = false">取消</button>
          <button class="primary" @click="confirmNewDir">创建</button>
        </div>
      </div>
    </div>

    <!-- 重命名对话框 -->
    <div v-if="showRename" class="modal-overlay" @click="showRename = false">
      <div class="modal" @click.stop>
        <h3>重命名</h3>
        <input v-model="renameNewName" @keyup.enter="confirmRename" autofocus />
        <div class="modal-actions">
          <button @click="showRename = false">取消</button>
          <button class="primary" @click="confirmRename">重命名</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.workspace-sidebar {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  overflow: hidden;
  user-select: none;
}

.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-panel);
}

.sidebar-title {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--fg-muted);
  letter-spacing: 0.5px;
  font-weight: 600;
}

.icon-btn {
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-button);
  cursor: pointer;
  font-size: 14px;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--fg-secondary);
}

.icon-btn:hover {
  background: var(--bg-button-hover);
  color: var(--fg-strong);
}

.tree-container {
  flex: 1;
  overflow: auto;
  padding: 4px 0;
}

.root-item {
  margin-bottom: 2px;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  cursor: pointer;
  font-size: 13px;
  color: var(--fg);
}

.tree-node:hover {
  background: var(--bg-hover);
}

.toggle {
  width: 12px;
  font-size: 10px;
  color: var(--fg-muted);
  text-align: center;
  flex-shrink: 0;
}

.file-icon {
  font-size: 14px;
  flex-shrink: 0;
}

.name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.root-name {
  font-weight: 600;
}

.remove-root-btn {
  border: none;
  background: none;
  cursor: pointer;
  color: var(--fg-muted);
  font-size: 16px;
  padding: 0 4px;
  opacity: 0;
  transition: opacity 0.15s;
}

.tree-node:hover .remove-root-btn {
  opacity: 1;
}

.remove-root-btn:hover {
  color: var(--danger);
}

.empty-dir {
  font-size: 11px;
  color: var(--fg-muted);
  font-style: italic;
  padding: 2px 0;
}

.context-menu {
  position: fixed;
  background: var(--bg-chrome);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 4px 12px var(--shadow);
  padding: 4px 0;
  z-index: 10000;
  min-width: 140px;
}

.menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 4px 12px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--fg);
}

.menu-item:hover {
  background: var(--bg-hover);
}

.menu-item.danger {
  color: var(--danger);
}

.menu-item.danger:hover {
  background: var(--danger-bg);
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10001;
}

.modal {
  background: var(--bg-chrome);
  border-radius: 8px;
  padding: 16px 20px;
  min-width: 320px;
  box-shadow: 0 8px 24px var(--shadow);
}

.modal h3 {
  font-size: 14px;
  margin-bottom: 12px;
  color: var(--fg);
}

.modal input {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  font-size: 13px;
  font-family: ui-monospace, monospace;
}

.modal input:focus {
  outline: none;
  border-color: var(--focus);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}

.modal-actions button {
  padding: 4px 12px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  cursor: pointer;
  font-size: 13px;
}

.modal-actions button.primary {
  background: var(--success);
  color: var(--success-fg);
  border-color: var(--success);
}

.modal-actions button.primary:hover {
  background: var(--success-hover);
}
</style>
