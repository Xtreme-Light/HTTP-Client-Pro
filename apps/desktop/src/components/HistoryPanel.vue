<script setup lang="ts">
import { computed } from 'vue';
import { useHistoryStore, type HistoryItem } from '../stores/history';
import { useRunCurrent } from '../composables/useRunCurrent';

const historyStore = useHistoryStore();
const { replay } = useRunCurrent();

const items = computed(() => historyStore.items);
const selectedId = computed(() => historyStore.selectedId);

const MAX_DISPLAY = 50;
const displayItems = computed(() => items.value.slice(0, MAX_DISPLAY));

function statusClass(s: number): string {
  if (s >= 200 && s < 300) return 'status-2xx';
  if (s >= 300 && s < 400) return 'status-3xx';
  if (s >= 400 && s < 500) return 'status-4xx';
  return 'status-5xx';
}

function methodClass(m: string): string {
  if (['GET', 'HEAD', 'OPTIONS'].includes(m)) return 'method-safe';
  if (['POST', 'PUT', 'PATCH'].includes(m)) return 'method-write';
  if (['DELETE', 'CONNECT', 'TRACE'].includes(m)) return 'method-danger';
  return 'method-unknown';
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString('en-US', { hour12: false });
}

/** 点击历史记录 — 选中以在下侧区域展示详情，不覆盖 Editor */
function onView(item: HistoryItem) {
  historyStore.selectItem(item.id);
}

/** 重放 — 重新发送请求 */
function onReplay(item: HistoryItem) {
  replay(item.source, item.method, item.target);
}

function onClear() {
  if (confirm('Clear all history?')) {
    historyStore.clear();
  }
}
</script>

<template>
  <div class="history-panel">
    <div class="panel-header">
      <h3>History</h3>
      <button v-if="items.length" class="clear-btn" @click="onClear">Clear</button>
    </div>

    <div v-if="displayItems.length === 0" class="empty-state">
      No history yet.
    </div>

    <div v-else class="history-list">
      <div
        v-for="item in displayItems"
        :key="item.id"
        :class="['history-item', { selected: selectedId === item.id }]"
        @click="onView(item)"
      >
        <div class="item-header">
          <span :class="['method-chip', methodClass(item.method)]">{{ item.method }}</span>
          <span :class="['status-chip', statusClass(item.status)]">{{ item.status }}</span>
          <span class="elapsed">{{ item.elapsedMs }}ms</span>
          <span class="time">{{ formatTime(item.ts) }}</span>
          <button
            class="replay-btn"
            title="Replay this request"
            @click.stop="onReplay(item)"
          >&#x21bb;</button>
        </div>
        <div class="item-target">{{ item.target }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.history-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
  overflow: hidden;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-panel);
}

.panel-header h3 {
  font-size: 12px;
  text-transform: uppercase;
  color: var(--fg-muted);
  letter-spacing: 0.5px;
}

.clear-btn {
  padding: 2px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-chrome);
  color: var(--danger);
  font-size: 11px;
  cursor: pointer;
}

.clear-btn:hover { background: var(--danger-bg); }

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--fg-muted);
  font-size: 13px;
}

.history-list {
  flex: 1;
  overflow: auto;
  padding: 4px;
}

.history-item {
  padding: 6px 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s;
  border-bottom: 1px solid var(--bg-chrome);
}

.history-item:hover {
  background: var(--bg-hover);
}

.history-item.selected {
  background: var(--focus-bg);
  border-left: 3px solid var(--focus);
}

.item-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 2px;
}

.method-chip {
  padding: 1px 6px;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 700;
}

.method-safe { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.method-write { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.method-danger { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }
.method-unknown { background: var(--chip-neutral-bg); color: var(--chip-neutral-fg); }

.status-chip {
  padding: 1px 6px;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 700;
}

.status-2xx { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.status-3xx { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.status-4xx { background: var(--chip-warn-bg); color: var(--chip-warn-fg); }
.status-5xx { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }

.elapsed {
  font-size: 11px;
  color: var(--fg-muted);
}

.time {
  font-size: 10px;
  color: var(--fg-muted);
  margin-left: auto;
}

.replay-btn {
  border: none;
  background: none;
  cursor: pointer;
  font-size: 14px;
  color: var(--fg-muted);
  padding: 0 4px;
  line-height: 1;
}

.replay-btn:hover {
  color: var(--focus);
}

.item-target {
  font-size: 11px;
  font-family: ui-monospace, monospace;
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
