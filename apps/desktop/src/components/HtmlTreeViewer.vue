<script setup lang="ts">
import { ref, computed, watch } from 'vue';

interface HtmlNode {
  type: 'element' | 'text' | 'comment';
  tag?: string;
  attrs?: Record<string, string>;
  text?: string;
  children: HtmlNode[];
  path: string;
}

const props = defineProps<{ html: string }>();

const collapsedMap = ref<Map<string, boolean>>(new Map());

watch(() => props.html, () => {
  collapsedMap.value = new Map();
});

const root = computed<HtmlNode | null>(() => {
  try {
    const parser = new DOMParser();
    const doc = parser.parseFromString(props.html, 'text/html');
    const node = buildFromDom(doc.documentElement, '$');
    return node;
  } catch {
    return null;
  }
});

function buildFromDom(el: Element, path: string): HtmlNode {
  const children: HtmlNode[] = [];
  const childElements = el.children;
  for (let i = 0; i < childElements.length; i++) {
    const child = childElements[i];
    children.push(buildFromDom(child, `${path}.${child.tagName.toLowerCase()}[${i}]`));
  }
  // 收集文本节点
  for (const node of el.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent?.trim();
      if (text) {
        children.push({
          type: 'text',
          text,
          children: [],
          path: `${path}.text`,
        });
      }
    } else if (node.nodeType === Node.COMMENT_NODE) {
      children.push({
        type: 'comment',
        text: node.textContent ?? '',
        children: [],
        path: `${path}.comment`,
      });
    }
  }
  const attrs: Record<string, string> = {};
  for (const attr of Array.from(el.attributes)) {
    attrs[attr.name] = attr.value;
  }
  return {
    type: 'element',
    tag: el.tagName.toLowerCase(),
    attrs,
    children,
    path,
  };
}

function isCollapsed(node: HtmlNode): boolean {
  return collapsedMap.value.get(node.path) ?? false;
}

function toggle(node: HtmlNode) {
  const current = collapsedMap.value.get(node.path) ?? false;
  const newMap = new Map(collapsedMap.value);
  newMap.set(node.path, !current);
  collapsedMap.value = newMap;
}

function collapseAll() {
  const newMap = new Map<string, boolean>();
  function walk(node: HtmlNode) {
    if (node.children.length > 0) {
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

function hasChildren(node: HtmlNode): boolean {
  return node.children.some(c => c.type === 'element') || node.children.length > 0;
}
</script>

<template>
  <div class="html-tree" v-if="root">
    <div class="tree-toolbar">
      <button class="tree-btn" @click="expandAll">Expand All</button>
      <button class="tree-btn" @click="collapseAll">Collapse All</button>
    </div>
    <div class="tree-node root-node">
      <span class="toggle" @click="toggle(root)">{{ isCollapsed(root) ? '▶' : '▼' }}</span>
      <span class="tag">&lt;{{ root.tag }}&gt;</span>
      <span v-if="root.attrs && Object.keys(root.attrs).length" class="attrs">
        <span v-for="(v, k) in root.attrs" :key="k" class="attr">
          <span class="attr-name">{{ k }}</span>=<span class="attr-value">"{{ v }}"</span>
        </span>
      </span>
      <div v-show="!isCollapsed(root)" class="children">
        <template v-for="child in root.children" :key="child.path">
          <div v-if="child.type === 'element'" class="tree-node">
            <span v-if="hasChildren(child)" class="toggle" @click="toggle(child)">
              {{ isCollapsed(child) ? '▶' : '▼' }}
            </span>
            <span v-else class="toggle-placeholder"></span>
            <span class="tag">&lt;{{ child.tag }}&gt;</span>
            <span v-if="child.attrs && Object.keys(child.attrs).length" class="attrs">
              <span v-for="(v, k) in child.attrs" :key="k" class="attr">
                <span class="attr-name">{{ k }}</span>=<span class="attr-value">"{{ v }}"</span>
              </span>
            </span>
            <div v-if="hasChildren(child) && !isCollapsed(child)" class="children">
              <template v-for="grandchild in child.children" :key="grandchild.path">
                <div v-if="grandchild.type === 'text'" class="text-node">{{ grandchild.text }}</div>
                <div v-else-if="grandchild.type === 'comment'" class="comment-node">&lt;!--{{ grandchild.text }}--&gt;</div>
              </template>
            </div>
            <span v-if="hasChildren(child) && isCollapsed(child)" class="collapsed-summary">…</span>
            <span class="tag-close">&lt;/{{ child.tag }}&gt;</span>
          </div>
          <div v-else-if="child.type === 'text'" class="text-node">{{ child.text }}</div>
          <div v-else-if="child.type === 'comment'" class="comment-node">&lt;!--{{ child.text }}--&gt;</div>
        </template>
      </div>
      <span v-if="isCollapsed(root)" class="collapsed-summary">…</span>
      <span class="tag-close">&lt;/{{ root.tag }}&gt;</span>
    </div>
  </div>
  <div v-else class="html-tree-error">
    Failed to parse HTML
  </div>
</template>

<style scoped>
.html-tree {
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

.tree-node { display: flex; align-items: baseline; flex-wrap: wrap; gap: 2px; }

.toggle {
  cursor: pointer;
  user-select: none;
  width: 14px;
  display: inline-block;
  text-align: center;
  color: var(--fg-muted);
}

.toggle:hover { color: var(--focus); }

.toggle-placeholder {
  display: inline-block;
  width: 14px;
}

.tag { color: var(--syn-tag); font-weight: 500; }
.tag-close { color: var(--syn-tag); }

.attrs { margin-left: 4px; }
.attr { margin-right: 4px; }
.attr-name { color: var(--syn-attr); }
.attr-value { color: var(--syn-string); }

.text-node {
  color: var(--fg-muted);
  margin-left: 20px;
  white-space: pre-wrap;
  word-break: break-word;
}

.comment-node {
  color: var(--syn-comment);
  font-style: italic;
  margin-left: 20px;
}

.collapsed-summary {
  color: var(--fg-muted);
  font-style: italic;
}

.children { margin-left: 20px; width: 100%; }

.html-tree-error {
  padding: 16px;
  color: var(--danger);
  font-size: 13px;
}
</style>
