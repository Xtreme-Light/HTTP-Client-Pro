<script setup lang="ts">
import { computed } from 'vue';
import { useRequestStore } from '../stores/request';
import { useEnvironmentStore } from '../stores/environment';
import { useSettingsStore } from '../stores/settings';
import { useRunCurrent } from '../composables/useRunCurrent';
import { formatActionBinding } from '../lib/keymaps';
import EnvironmentEditor from './EnvironmentEditor.vue';

const requestStore = useRequestStore();
const envStore = useEnvironmentStore();
const settings = useSettingsStore();
const { run } = useRunCurrent();

const currentBlock = computed(() => requestStore.currentBlock);

// Run 按钮快捷键提示 — 随快捷键方案变化
const runShortcut = computed(() => formatActionBinding(settings.keymapScheme, 'runRequest'));

const methodClass = computed(() => {
  const m = currentBlock.value?.method?.toUpperCase();
  if (!m) return 'method-unknown';
  if (['GET', 'HEAD', 'OPTIONS'].includes(m)) return 'method-safe';
  if (['POST', 'PUT', 'PATCH'].includes(m)) return 'method-write';
  if (['DELETE', 'CONNECT', 'TRACE'].includes(m)) return 'method-danger';
  return 'method-unknown';
});
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <span :class="['method-chip', methodClass]">
        {{ currentBlock?.method ?? '—' }}
      </span>
      <span class="target">{{ currentBlock?.target ?? 'No request' }}</span>
      <span v-if="currentBlock?.name" class="block-name">— {{ currentBlock.name }}</span>
    </div>

    <div class="toolbar-right">
      <EnvironmentEditor />
      <button
        class="run-btn"
        :disabled="!currentBlock"
        @click="run()"
      >
        ▶ Run ({{ runShortcut }})
      </button>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  gap: 12px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  flex: 1;
}

.method-chip {
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.method-safe { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.method-write { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.method-danger { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }
.method-unknown { background: var(--chip-neutral-bg); color: var(--chip-neutral-fg); }

.target {
  font-family: ui-monospace, monospace;
  font-size: 12px;
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.block-name {
  color: var(--syn-keyword);
  font-size: 12px;
  white-space: nowrap;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.run-btn {
  padding: 4px 16px;
  border: 1px solid var(--success);
  border-radius: 6px;
  background: var(--success);
  color: var(--success-fg);
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s;
}

.run-btn:hover:not(:disabled) {
  background: var(--success-hover);
}

.run-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
