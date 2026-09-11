<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useHistoryStore } from '../stores/history';
import { useResponseStore } from '../stores/response';
import { useSettingsStore } from '../stores/settings';
import { formatActionBinding } from '../lib/keymaps';

const historyStore = useHistoryStore();
const responseStore = useResponseStore();
const settings = useSettingsStore();

// 运行快捷键拆分（每个按键渲染为 <kbd>）— 随快捷键方案变化
const runShortcutParts = computed(() =>
  formatActionBinding(settings.keymapScheme, 'runRequest').split('+'),
);

type Tab = 'console' | 'request';
const activeTab = ref<Tab>('console');

// 当选中的历史记录变化时，默认切换到 console tab
watch(
  () => historyStore.selectedId,
  () => { activeTab.value = 'console'; },
);

const item = computed(() => historyStore.selectedItem);

function statusClass(s: number): string {
  if (s >= 200 && s < 300) return 'status-2xx';
  if (s >= 300 && s < 400) return 'status-3xx';
  if (s >= 400 && s < 500) return 'status-4xx';
  return 'status-5xx';
}

// console 文本：展示完整的请求内容（method + target + headers + body）
const consoleText = computed(() => {
  if (!item.value) return '';
  const lines: string[] = [];
  lines.push(`${item.value.method} ${item.value.target}`);
  lines.push('');
  lines.push(`Status: ${item.value.status}  |  Elapsed: ${item.value.elapsedMs}ms`);
  lines.push('');
  if (item.value.response) {
    const res = item.value.response;
    lines.push('--- Response ---');
    lines.push(`Status: ${res.status}  |  Elapsed: ${res.elapsed_ms}ms  |  URL: ${res.url}`);
    lines.push('');
    const headers = res.headers ?? {};
    const ct = (headers['content-type'] ?? headers['Content-Type'] ?? '');
    if (ct.toLowerCase().includes('json')) {
      try {
        lines.push(JSON.stringify(JSON.parse(res.body), null, 2));
      } catch {
        lines.push(res.body);
      }
    } else {
      lines.push(res.body);
    }
  }
  return lines.join('\n');
});

// request 文本：展示历史请求的 Editor 源码
const requestText = computed(() => item.value?.source ?? '');
</script>

<template>
  <div class="response-pane">
    <!-- Loading -->
    <div v-if="responseStore.loading" class="state loading">
      <span class="spinner" /> Sending request…
    </div>

    <!-- Error -->
    <div v-else-if="responseStore.error" class="state error">
      <strong>Error</strong>
      <pre>{{ responseStore.error }}</pre>
    </div>

    <!-- Detail content -->
    <div v-else-if="item" class="response-content">
      <!-- Meta bar -->
      <div class="response-meta">
        <span
          v-if="item.response"
          :class="['status-chip', statusClass(item.response.status)]"
        >{{ item.response.status }}</span>
        <span class="elapsed">{{ item.elapsedMs }}ms</span>
        <span class="url">{{ item.target }}</span>
      </div>

      <!-- Tabs -->
      <div class="tabs">
        <button :class="['tab', { active: activeTab === 'console' }]" @click="activeTab = 'console'">
          Console
        </button>
        <button :class="['tab', { active: activeTab === 'request' }]" @click="activeTab = 'request'">
          Request
        </button>
      </div>

      <!-- Console tab -->
      <div v-show="activeTab === 'console'" class="tab-content">
        <pre class="console-viewer">{{ consoleText }}</pre>
      </div>

      <!-- Request tab -->
      <div v-show="activeTab === 'request'" class="tab-content">
        <pre class="request-viewer">{{ requestText }}</pre>
      </div>
    </div>

    <!-- Idle -->
    <div v-else class="state idle">
      <div class="idle-content">
        <p>
          Press
          <template v-for="(p, i) in runShortcutParts" :key="i">
            <kbd>{{ p }}</kbd><span v-if="i < runShortcutParts.length - 1">+</span>
          </template>
          or click <strong>Send</strong> to execute a request.
        </p>
        <p>Or select an item from <strong>History</strong> to view its response.</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.response-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
  overflow: hidden;
}

.state {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 24px;
  color: var(--fg-muted);
}

.state.idle {
  justify-content: center;
  text-align: center;
  flex-direction: column;
  flex: 1;
}

.state.loading { color: var(--focus); }

.state.error {
  flex-direction: column;
  align-items: flex-start;
  color: var(--danger);
}

.state.error pre {
  margin: 4px 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border-strong);
  border-top-color: var(--focus);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.response-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.response-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
}

.status-chip {
  padding: 2px 10px;
  border-radius: 12px;
  font-weight: 700;
  font-size: 13px;
}

.status-2xx { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.status-3xx { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.status-4xx { background: var(--chip-warn-bg); color: var(--chip-warn-fg); }
.status-5xx { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }

.elapsed { font-size: 12px; color: var(--fg-muted); }

.url {
  font-family: ui-monospace, monospace;
  font-size: 11px;
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.tabs {
  display: flex;
  border-bottom: 1px solid var(--border);
  background: var(--bg-panel);
}

.tab {
  padding: 4px 12px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--fg-muted);
  border-bottom: 2px solid transparent;
  transition: all 0.15s;
}

.tab:hover { color: var(--fg); }

.tab.active {
  color: var(--focus);
  border-bottom-color: var(--focus);
  background: var(--bg-base);
}

.tab-content {
  flex: 1;
  overflow: auto;
  padding: 8px;
}

.console-viewer,
.request-viewer {
  background: var(--bg-input-deep);
  border: 1px solid var(--border-block);
  border-radius: 6px;
  padding: 8px;
  overflow: auto;
  font-size: 12px;
  font-family: var(--font-editor);
  white-space: pre-wrap;
  margin: 0;
  min-height: 100%;
  box-sizing: border-box;
}

.idle-content p {
  color: var(--fg-muted);
  font-size: 14px;
}

kbd {
  padding: 2px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  font-size: 12px;
  background: var(--bg-panel);
}
</style>
