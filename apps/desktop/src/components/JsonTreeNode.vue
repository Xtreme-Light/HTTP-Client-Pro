<script setup lang="ts">
import { computed } from 'vue';
import type { TreeNode } from './JsonTreeViewer.vue';

const props = defineProps<{
  node: TreeNode;
  collapsedMap: Map<string, boolean>;
}>();

const emit = defineEmits<{
  toggle: [node: TreeNode];
  'copy-path': [node: TreeNode];
}>();

const isCollapsed = computed(() => props.collapsedMap.get(props.node.path) ?? false);

function formatValue(node: TreeNode): string {
  switch (node.type) {
    case 'string': return '"' + node.value + '"';
    case 'number': case 'boolean': return String(node.value);
    case 'null': return 'null';
    default: return '';
  }
}

function getTypeBadge(node: TreeNode): string {
  if (node.type === 'array') return '[' + node.children.length + ']';
  if (node.type === 'object') return '{' + node.children.length + '}';
  return '';
}
</script>

<template>
  <div class="tree-node">
    <span v-if="node.type === 'object' || node.type === 'array'" class="toggle" @click="emit('toggle', node)">
      {{ isCollapsed ? '▶' : '▼' }}
    </span>
    <span v-else class="toggle-placeholder"></span>

    <span class="key">{{ node.key }}:</span>

    <span v-if="node.type === 'object' || node.type === 'array'" class="type-badge">
      {{ getTypeBadge(node) }}
    </span>
    <span v-else class="value" :class="node.type">{{ formatValue(node) }}</span>

    <button class="copy-btn" @click="emit('copy-path', node)" title="Copy JSON path">⎘</button>

    <div v-if="!isCollapsed && (node.type === 'object' || node.type === 'array')" class="children">
      <JsonTreeNode
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :collapsed-map="collapsedMap"
        @toggle="emit('toggle', $event)"
        @copy-path="emit('copy-path', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.tree-node {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 4px;
}

.toggle {
  cursor: pointer;
  user-select: none;
  width: 14px;
  text-align: center;
  color: var(--fg-muted);
  display: inline-block;
}

.toggle:hover {
  color: var(--focus);
}

.toggle-placeholder {
  display: inline-block;
  width: 14px;
}

.key { color: var(--syn-url); font-weight: 500; }
.type-badge { color: var(--fg-muted); font-size: 11px; }

.value.string { color: var(--syn-string); }
.value.number { color: var(--syn-number); }
.value.boolean { color: var(--syn-bool); }
.value.null { color: var(--fg-muted); font-style: italic; }

.copy-btn {
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--fg-muted);
  opacity: 0;
  transition: opacity 0.15s;
}

.tree-node:hover .copy-btn { opacity: 1; }

.children { margin-left: 20px; width: 100%; }
</style>
