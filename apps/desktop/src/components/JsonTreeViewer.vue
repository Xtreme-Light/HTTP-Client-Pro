<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import JsonTreeNode from './JsonTreeNode.vue';

export interface TreeNode {
  key: string;
  value: unknown;
  path: string;
  type: 'object' | 'array' | 'string' | 'number' | 'boolean' | 'null';
  children: TreeNode[];
}

const props = defineProps<{ data: unknown }>();

/** 每个节点的折叠状态，按 path 索引 */
const collapsedMap = ref<Map<string, boolean>>(new Map());

/** 当 data 变化时重置折叠状态 */
watch(() => props.data, () => {
  collapsedMap.value = new Map();
}, { deep: false });

const root = computed<TreeNode | null>(() => {
  return buildNode('', props.data, '$');
});

function buildNode(key: string, value: unknown, path: string): TreeNode {
  const type = getType(value);
  const children: TreeNode[] = [];

  if (type === 'object' && value !== null) {
    const obj = value as Record<string, unknown>;
    for (const [k, v] of Object.entries(obj)) {
      children.push(buildNode(k, v, `${path}.${k}`));
    }
  } else if (type === 'array') {
    const arr = value as unknown[];
    arr.forEach((v, i) => {
      children.push(buildNode(String(i), v, `${path}[${i}]`));
    });
  }

  return { key, value, path, type, children };
}

function getType(v: unknown): TreeNode['type'] {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  return typeof v as TreeNode['type'];
}

function isCollapsed(node: TreeNode): boolean {
  return collapsedMap.value.get(node.path) ?? false;
}

function toggle(node: TreeNode) {
  const current = collapsedMap.value.get(node.path) ?? false;
  // 使用新 Map 触发 Vue 响应式
  const newMap = new Map(collapsedMap.value);
  newMap.set(node.path, !current);
  collapsedMap.value = newMap;
}

function collapseAll() {
  const newMap = new Map<string, boolean>();
  function walk(node: TreeNode) {
    if (node.type === 'object' || node.type === 'array') {
      newMap.set(node.path, true);
      node.children.forEach(walk);
    }
  }
  if (root.value) walk(root.value);
  collapsedMap.value = newMap;
}

function expandAll() {
  collapsedMap.value = new Map();
}

function formatValue(node: TreeNode): string {
  switch (node.type) {
    case 'string': return '"' + node.value + '"';
    case 'number': case 'boolean': return String(node.value);
    case 'null': return 'null';
    default: return '';
  }
}

function copyPath(node: TreeNode) {
  const path = node.path.replace(/^\$\./, '').replace(/^\$/, '');
  navigator.clipboard?.writeText(path);
}

function badgeText(node: TreeNode): string {
  if (node.type === 'array') return '[' + node.children.length + ']';
  if (node.type === 'object') return '{' + node.children.length + '}';
  return '';
}
</script>

<template>
  <div class="json-tree" v-if="root">
    <div class="tree-toolbar">
      <button class="tree-btn" @click="expandAll">Expand All</button>
      <button class="tree-btn" @click="collapseAll">Collapse All</button>
    </div>
    <div
      v-if="root.type === 'object' || root.type === 'array'"
      class="tree-node root-node"
    >
      <span class="toggle" @click="toggle(root)">{{ isCollapsed(root) ? '▶' : '▼' }}</span>
      <span class="key">{{ root.key || 'root' }}</span>
      <span class="type-badge">{{ badgeText(root) }}</span>
      <div v-show="!isCollapsed(root)" class="children">
        <JsonTreeNode
          v-for="child in root.children"
          :key="child.path"
          :node="child"
          :collapsed-map="collapsedMap"
          @toggle="toggle"
          @copy-path="copyPath"
        />
      </div>
    </div>
    <div v-else class="tree-node leaf">
      <span class="value" :class="root.type">{{ formatValue(root) }}</span>
    </div>
  </div>
</template>

<style scoped>
.json-tree {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
  line-height: 1.6;
  background: var(--bg-input-deep);
  border: 1px solid var(--border-block);
  border-radius: 4px;
  padding: 8px;
  overflow: auto;
  max-height: 100%;
}

.tree-toolbar {
  display: flex;
  gap: 4px;
  margin-bottom: 4px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-block);
}

.tree-btn {
  padding: 1px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--fg-secondary);
  font-size: 11px;
  cursor: pointer;
}

.tree-btn:hover {
  background: var(--bg-button);
  color: var(--fg-strong);
}

.tree-node { display: flex; align-items: center; gap: 4px; }

.toggle {
  cursor: pointer;
  user-select: none;
  width: 14px;
  display: inline-block;
  text-align: center;
  color: var(--fg-muted);
}

.toggle:hover {
  color: var(--focus);
}

.key { color: var(--syn-url); font-weight: 500; }
.type-badge { color: var(--fg-muted); font-size: 11px; }

.value.string { color: var(--syn-string); }
.value.number { color: var(--syn-number); }
.value.boolean { color: var(--syn-bool); }
.value.null { color: var(--fg-muted); font-style: italic; }

.children { margin-left: 20px; }
.leaf { padding-left: 18px; }
</style>
