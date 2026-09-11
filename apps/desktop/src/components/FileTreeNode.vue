<script setup lang="ts">
import type { FileEntry } from '../stores/workspace';

const props = defineProps<{
  entry: FileEntry;
  expandedDirs: Set<string>;
  dirCache: Map<string, FileEntry[]>;
  level?: number;
}>();

defineEmits<{
  'toggle-dir': [path: string];
  'file-click': [entry: FileEntry];
  'context-menu': [entry: FileEntry, event: MouseEvent];
}>();

const TEXT_EXTENSIONS = ['.http', '.md', '.txt', '.json', '.yaml', '.yml', '.xml', '.csv', '.log', '.ts', '.js', '.vue', '.rs'];

function isTextFile(name: string): boolean {
  const lower = name.toLowerCase();
  return TEXT_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

function getEntries(path: string): FileEntry[] {
  return props.dirCache.get(path) ?? [];
}
</script>

<template>
  <div class="tree-node-wrapper">
    <div
      class="tree-node"
      :style="{ paddingLeft: (props.level ?? 1) * 16 + 4 + 'px' }"
      @click="$emit('file-click', props.entry)"
      @contextmenu.prevent.stop="$emit('context-menu', props.entry, $event)"
    >
      <span v-if="entry.isDir" class="toggle">
        {{ expandedDirs.has(entry.path) ? '▼' : '▶' }}
      </span>
      <span v-else class="toggle-placeholder"></span>
      <span class="file-icon" :class="entry.isDir ? 'folder-icon' : 'file-icon-leaf'">
        {{ entry.isDir ? '📁' : (isTextFile(entry.name) ? '📄' : '⚙') }}
      </span>
      <span class="name">{{ entry.name }}</span>
    </div>
    <div v-if="entry.isDir && expandedDirs.has(entry.path)" class="children">
      <FileTreeNode
        v-for="child in getEntries(entry.path)"
        :key="child.path"
        :entry="child"
        :expanded-dirs="expandedDirs"
        :dir-cache="dirCache"
        :level="(props.level ?? 1) + 1"
        @toggle-dir="(path) => $emit('toggle-dir', path)"
        @file-click="(child) => $emit('file-click', child)"
        @context-menu="(child, event) => $emit('context-menu', child, event)"
      />
      <div v-if="getEntries(entry.path).length === 0" class="empty-dir" :style="{ paddingLeft: (props.level ?? 1) * 16 + 24 + 'px' }">
        (empty)
      </div>
    </div>
  </div>
</template>

<style scoped>
.tree-node-wrapper {
  /* wrapper for recursive node */
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

.toggle-placeholder {
  width: 12px;
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

.empty-dir {
  font-size: 11px;
  color: var(--fg-muted);
  font-style: italic;
  padding: 2px 0;
}

.children {
  /* nothing special */
}
</style>
