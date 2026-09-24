<script setup lang="ts">
/**
 * ResponsePane — 展示选中请求的最近一次（或钉住的历史）运行结果。
 * 执行中显示转圈，失败显示错误，成功显示 Console / Request 两个页签。
 */
import { computed, ref, watch } from 'vue';
import { useRequestsStore } from '../stores/requests';
import { useSettingsStore } from '../stores/settings';
import { formatActionBinding } from '../lib/keymaps';
import { formatConsole } from '../lib/console-format';

const requestsStore = useRequestsStore();
const settings = useSettingsStore();

// 运行快捷键拆分（每个按键渲染为 <kbd>）— 随快捷键方案变化
const runShortcutParts = computed(() =>
  formatActionBinding(settings.keymapScheme, 'runRequest').split('+'),
);

type Tab = 'console' | 'request';
const activeTab = ref<Tab>('console');

const entry = computed(() => requestsStore.selectedEntry);
const run = computed(() => requestsStore.selectedRun);

// 切换选中的请求 / 运行记录时，默认回到 console 页签
watch(
  () => [requestsStore.selectedRequestId, requestsStore.selectedRunId],
  () => { activeTab.value = 'console'; },
);

function statusClass(s: number): string {
  if (s >= 200 && s < 300) return 'status-2xx';
  if (s >= 300 && s < 400) return 'status-3xx';
  if (s >= 400 && s < 500) return 'status-4xx';
  return 'status-5xx';
}

// console 文本：JetBrains 风格（请求行 + 状态行 + 完整响应头 + body/落盘 + 摘要）
const consoleText = computed(() => {
  const r = run.value;
  if (!r) return '';
  return formatConsole({
    method: r.method,
    target: r.target,
    response: r.response,
  });
});

// request 文本：该次运行发起时的请求源码快照
const requestText = computed(() => run.value?.source ?? '');

/** 正在查看钉住的历史运行（而非最近一次） */
const viewingHistory = computed(
  () => !!run.value && !!entry.value && run.value.runId !== entry.value.runs[0]?.runId,
);

function backToLatest() {
  if (!entry.value) return;
  requestsStore.viewRun(entry.value.requestId, null);
}
</script>

<template>
  <div class="response-pane">
    <!-- 执行中 -->
    <div v-if="run && run.status === 'running'" class="state loading">
      <span class="spinner" /> 请求执行中…
    </div>

    <!-- 失败 -->
    <div v-else-if="run && run.status === 'error'" class="state error">
      <div v-if="viewingHistory" class="history-bar">
        <span>正在查看历史运行</span>
        <button class="link-btn" @click="backToLatest">回到最近一次</button>
      </div>
      <strong>错误</strong>
      <pre>{{ run.error }}</pre>
    </div>

    <!-- 成功 -->
    <div v-else-if="run" class="response-content">
      <div v-if="viewingHistory" class="history-bar">
        <span>正在查看历史运行（{{ new Date(run.ts).toLocaleTimeString('zh-CN', { hour12: false }) }}）</span>
        <button class="link-btn" @click="backToLatest">回到最近一次</button>
      </div>

      <!-- Meta bar -->
      <div class="response-meta">
        <span
          v-if="run.httpStatus != null"
          :class="['status-chip', statusClass(run.httpStatus)]"
        >{{ run.httpStatus }}</span>
        <span class="elapsed">{{ run.elapsedMs }}ms</span>
        <span class="url">{{ run.target }}</span>
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

    <!-- 空闲 -->
    <div v-else class="state idle">
      <div class="idle-content">
        <p>
          按
          <template v-for="(p, i) in runShortcutParts" :key="i">
            <kbd>{{ p }}</kbd><span v-if="i < runShortcutParts.length - 1">+</span>
          </template>
          或点击编辑器中的 <strong>&#9654;</strong> 执行请求。
        </p>
        <p>或在 <strong>Requests</strong> 面板中选择一个请求查看其响应。</p>
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
  flex: 1;
  overflow: auto;
}

.state.error pre {
  margin: 4px 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.spinner {
  width: 16px;
  height: 16px;
  flex: none;
  border: 2px solid var(--border-strong);
  border-top-color: var(--focus);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.history-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  box-sizing: border-box;
  padding: 4px 12px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
}

.link-btn {
  border: none;
  background: none;
  color: var(--focus);
  cursor: pointer;
  font-size: calc(11px * var(--font-scale, 1));
  padding: 0;
}

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
  font-size: calc(13px * var(--font-scale, 1));
}

.status-2xx { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.status-3xx { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.status-4xx { background: var(--chip-warn-bg); color: var(--chip-warn-fg); }
.status-5xx { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }

.elapsed { font-size: calc(12px * var(--font-scale, 1)); color: var(--fg-muted); }

.url {
  font-family: var(--font-editor);
  font-size: calc(11px * var(--font-scale, 1));
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
  font-size: calc(12px * var(--font-scale, 1));
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
  font-size: calc(12px * var(--font-scale, 1));
  /* 跟随界面字体设置（--font-ui），与 REQUESTS 等面板保持一致 */
  font-family: var(--font-ui);
  white-space: pre-wrap;
  margin: 0;
  min-height: 100%;
  box-sizing: border-box;
}

.idle-content p {
  color: var(--fg-muted);
  font-size: calc(14px * var(--font-scale, 1));
}

kbd {
  padding: 2px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  font-size: calc(12px * var(--font-scale, 1));
  background: var(--bg-panel);
}
</style>
