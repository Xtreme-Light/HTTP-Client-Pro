<script setup lang="ts">
/**
 * RequestPanel — 「请求集合」面板（取代旧的 History 面板）。
 *
 * 每行是一个请求（同一文件里的同一个 block），发起即出现并显示转圈；
 * 同一请求的反复执行归并到自己名下，右侧详情展示最近一次运行。
 * 选中某行后展开操作栏：执行 / 调试 / 配置；行右上角时钟查看该请求的历史运行。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useRequestsStore, type RequestEntry, type RunRecord } from '../stores/requests';
import { setParallelConfirmHandler, useRunCurrent } from '../composables/useRunCurrent';

const requestsStore = useRequestsStore();
const { rerun } = useRunCurrent();

const entries = computed(() => requestsStore.entries);
const selectedId = computed(() => requestsStore.selectedRequestId);

/** 历史下拉展开的请求 ID */
const historyOpenId = ref<string | null>(null);
/** 「修改运行配置」弹窗目标 */
const configTarget = ref<RequestEntry | null>(null);
const cfgName = ref('');
const cfgParallel = ref(false);
/** 调试信息弹窗目标 */
const debugTarget = ref<RequestEntry | null>(null);
/** 并行询问弹窗 */
const parallelMessage = ref<string | null>(null);
let parallelResolve: ((allow: boolean) => void) | null = null;

onMounted(() => {
  setParallelConfirmHandler((p) => {
    parallelMessage.value = p.message;
    parallelResolve = p.resolve;
  });
});

onUnmounted(() => {
  setParallelConfirmHandler(null);
  parallelResolve?.(false);
  parallelResolve = null;
});

function answerParallel(allow: boolean) {
  parallelMessage.value = null;
  const resolve = parallelResolve;
  parallelResolve = null;
  resolve?.(allow);
}

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
  return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false });
}

function displayName(entry: RequestEntry): string {
  return entry.name ?? entry.target ?? '(未命名请求)';
}

function onSelect(entry: RequestEntry) {
  requestsStore.select(entry.requestId);
  historyOpenId.value = null;
}

function onRerun(entry: RequestEntry) {
  void rerun(entry.requestId);
}

function toggleHistory(entry: RequestEntry) {
  historyOpenId.value = historyOpenId.value === entry.requestId ? null : entry.requestId;
}

/** 查看某次运行 — runId 为 null 表示回到最近一次 */
function onViewRun(entry: RequestEntry, run: RunRecord | null) {
  requestsStore.viewRun(entry.requestId, run ? run.runId : null);
  historyOpenId.value = null;
}

function openConfig(entry: RequestEntry) {
  configTarget.value = entry;
  cfgName.value = entry.name ?? '';
  cfgParallel.value = entry.allowParallel;
  historyOpenId.value = null;
}

function saveConfig() {
  const entry = configTarget.value;
  if (!entry) return;
  requestsStore.updateConfig(entry.requestId, {
    name: cfgName.value,
    allowParallel: cfgParallel.value,
  });
  configTarget.value = null;
}

function onClear() {
  if (confirm('清空所有请求记录？')) {
    requestsStore.clear();
  }
}
</script>

<template>
  <div class="request-panel">
    <div class="panel-header">
      <h3>Requests</h3>
      <button v-if="entries.length" class="clear-btn" @click="onClear">Clear</button>
    </div>

    <div v-if="requestsStore.notice" class="notice">
      <span>{{ requestsStore.notice }}</span>
      <button class="notice-close" @click="requestsStore.setNotice(null)">&times;</button>
    </div>

    <div v-if="entries.length === 0" class="empty-state">
      尚无请求 — 在编辑器中点击 <strong>&#9654;</strong> 或按快捷键执行请求。
    </div>

    <div v-else class="request-list">
      <div
        v-for="entry in entries"
        :key="entry.requestId"
        :class="['request-item', { selected: selectedId === entry.requestId }]"
      >
        <div class="item-main" @click="onSelect(entry)">
          <span v-if="entry.runs[0]?.status === 'running'" class="spinner" />
          <span
            v-else-if="entry.runs[0]?.status === 'done' && entry.runs[0].httpStatus != null"
            :class="['status-chip', statusClass(entry.runs[0].httpStatus)]"
          >{{ entry.runs[0].httpStatus }}</span>
          <span v-else class="status-chip status-err">&#10005;</span>

          <span :class="['method-chip', methodClass(entry.method)]">{{ entry.method }}</span>

          <span class="item-text">
            <span class="item-name">{{ displayName(entry) }}</span>
            <span class="item-target">{{ entry.target }}</span>
          </span>

          <span v-if="entry.allowParallel" class="parallel-flag" title="已允许并行请求">&#8646;</span>

          <button
            class="icon-btn"
            title="查看该请求的历史"
            @click.stop="toggleHistory(entry)"
          >
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3">
              <circle cx="8" cy="8" r="6" />
              <path d="M8 4.6V8l2.4 1.4" stroke-linecap="round" />
            </svg>
          </button>
        </div>

        <!-- 历史运行下拉 -->
        <div v-if="historyOpenId === entry.requestId" class="history-pop">
          <div class="history-pop-title">
            执行历史（{{ entry.runs.length }}）
            <button class="link-btn" @click="onViewRun(entry, null)">回到最近一次</button>
          </div>
          <div
            v-for="run in entry.runs"
            :key="run.runId"
            class="history-row"
            :class="{ pinned: requestsStore.selectedRunId === run.runId }"
            @click="onViewRun(entry, run)"
          >
            <span v-if="run.status === 'running'" class="spinner small" />
            <span
              v-else-if="run.status === 'done' && run.httpStatus != null"
              :class="['status-chip', statusClass(run.httpStatus)]"
            >{{ run.httpStatus }}</span>
            <span v-else class="status-chip status-err">&#10005;</span>
            <span class="history-time">{{ formatTime(run.ts) }}</span>
            <span v-if="run.status === 'done'" class="history-elapsed">{{ run.elapsedMs }}ms</span>
            <span v-else-if="run.status === 'error'" class="history-elapsed err">失败</span>
            <span v-else class="history-elapsed">执行中</span>
          </div>
        </div>

        <!-- 选中后展开的操作栏 -->
        <div v-if="selectedId === entry.requestId" class="action-bar">
          <button class="act-btn run" title="执行该请求" @click.stop="onRerun(entry)">
            <svg viewBox="0 0 16 16" fill="currentColor"><path d="M4 2.2 13 8l-9 5.8z" /></svg>
          </button>
          <button class="act-btn" title="查看请求信息" @click.stop="debugTarget = entry; historyOpenId = null">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
              <rect x="5.4" y="4.4" width="5.2" height="8.2" rx="2.6" />
              <path d="M5.4 7H2.6M5.4 10H2.6M10.6 7h2.8M10.6 10h2.8M6.6 4.4 5.2 2.4M9.4 4.4l1.4-2" />
            </svg>
          </button>
          <button class="act-btn" title="修改运行配置" @click.stop="openConfig(entry)">
            <svg viewBox="0 0 16 16" fill="currentColor">
              <circle cx="8" cy="3" r="1.4" /><circle cx="8" cy="8" r="1.4" /><circle cx="8" cy="13" r="1.4" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <!-- 修改运行配置 -->
    <div v-if="configTarget" class="modal-overlay" @click="configTarget = null">
      <div class="modal" @click.stop>
        <h3>修改运行配置</h3>

        <label class="field-label" for="cfg-name">名称</label>
        <input
          id="cfg-name"
          v-model="cfgName"
          class="field-input"
          type="text"
          placeholder="默认取 ### 后的请求名"
        />

        <label class="check-row">
          <input v-model="cfgParallel" type="checkbox" />
          <span>允许并行请求</span>
        </label>
        <p class="field-hint">未勾选时，若该请求仍在执行中，再次执行会先询问是否并行发送。</p>

        <div class="modal-actions">
          <button class="btn-cancel" @click="configTarget = null">取消</button>
          <button class="btn-save" @click="saveConfig">保存</button>
        </div>
      </div>
    </div>

    <!-- 请求信息（调试） -->
    <div v-if="debugTarget" class="modal-overlay" @click="debugTarget = null">
      <div class="modal" @click.stop>
        <h3>请求信息</h3>
        <table class="info-table">
          <tbody>
            <tr><th>名称</th><td>{{ displayName(debugTarget) }}</td></tr>
            <tr><th>方法</th><td>{{ debugTarget.method }}</td></tr>
            <tr><th>目标</th><td class="mono">{{ debugTarget.target }}</td></tr>
            <tr><th>文件</th><td class="mono">{{ debugTarget.filePath ?? '（未保存）' }}</td></tr>
            <tr><th>允许并行请求</th><td>{{ debugTarget.allowParallel ? '是' : '否' }}</td></tr>
            <tr><th>执行次数</th><td>{{ debugTarget.runs.length }}</td></tr>
            <tr v-if="debugTarget.runs[0]">
              <th>最近一次</th>
              <td>
                {{ formatTime(debugTarget.runs[0].ts) }}
                <template v-if="debugTarget.runs[0].status === 'done'">
                  · {{ debugTarget.runs[0].httpStatus }} · {{ debugTarget.runs[0].elapsedMs }}ms
                </template>
                <template v-else-if="debugTarget.runs[0].status === 'running'">· 执行中</template>
                <template v-else>· 失败</template>
              </td>
            </tr>
            <tr v-if="debugTarget.runs[0]?.error">
              <th>错误</th><td class="err">{{ debugTarget.runs[0].error }}</td>
            </tr>
          </tbody>
        </table>

        <label class="field-label">请求源码</label>
        <pre class="source-view">{{ debugTarget.runs[0]?.source || '（无）' }}</pre>

        <div class="modal-actions">
          <button class="btn-cancel" @click="debugTarget = null">关闭</button>
        </div>
      </div>
    </div>

    <!-- 并行询问 -->
    <div v-if="parallelMessage" class="modal-overlay" @click="answerParallel(false)">
      <div class="modal narrow" @click.stop>
        <h3>并行请求</h3>
        <p class="modal-text">{{ parallelMessage }}</p>
        <div class="modal-actions">
          <button class="btn-cancel" @click="answerParallel(false)">取消</button>
          <button class="btn-save" @click="answerParallel(true)">并行发送</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.request-panel {
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
  font-size: calc(12px * var(--font-scale, 1));
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
  font-size: calc(11px * var(--font-scale, 1));
  cursor: pointer;
}

.clear-btn:hover { background: var(--danger-bg); }

/* ---- 提示条 ---- */
.notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  background: var(--danger-bg);
  color: var(--danger);
  font-size: calc(11px * var(--font-scale, 1));
  border-bottom: 1px solid var(--border);
}

.notice span { flex: 1; word-break: break-word; }

.notice-close {
  border: none;
  background: none;
  color: inherit;
  cursor: pointer;
  font-size: calc(14px * var(--font-scale, 1));
  line-height: 1;
  padding: 0 2px;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  padding: 16px;
  text-align: center;
  color: var(--fg-muted);
  font-size: calc(13px * var(--font-scale, 1));
}

.request-list {
  flex: 1;
  overflow: auto;
  padding: 4px;
}

.request-item {
  border-radius: 4px;
  cursor: pointer;
  border-bottom: 1px solid var(--bg-chrome);
}

.request-item:hover { background: var(--bg-hover); }

.request-item.selected {
  background: var(--focus-bg);
  border-left: 3px solid var(--focus);
}

.item-main {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
}

.item-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.item-name {
  font-size: calc(12px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-target {
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.parallel-flag {
  color: var(--fg-muted);
  font-size: calc(13px * var(--font-scale, 1));
  line-height: 1;
}

/* ---- 图标按钮 ---- */
.icon-btn {
  border: none;
  background: none;
  cursor: pointer;
  color: var(--fg-muted);
  padding: 2px;
  line-height: 0;
  border-radius: 3px;
}

.icon-btn:hover { color: var(--focus); background: var(--bg-chrome); }

.icon-btn svg {
  width: calc(14px * var(--font-scale, 1));
  height: calc(14px * var(--font-scale, 1));
}

/* ---- chips ---- */
.method-chip,
.status-chip {
  padding: 1px 6px;
  border-radius: 8px;
  font-size: calc(10px * var(--font-scale, 1));
  font-weight: 700;
  flex: none;
}

.method-safe { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.method-write { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.method-danger { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }
.method-unknown { background: var(--chip-neutral-bg); color: var(--chip-neutral-fg); }

.status-2xx { background: var(--chip-safe-bg); color: var(--chip-safe-fg); }
.status-3xx { background: var(--chip-write-bg); color: var(--chip-write-fg); }
.status-4xx { background: var(--chip-warn-bg); color: var(--chip-warn-fg); }
.status-5xx { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }
.status-err { background: var(--chip-danger-bg); color: var(--chip-danger-fg); }

/* ---- spinner ---- */
.spinner {
  width: 12px;
  height: 12px;
  flex: none;
  border: 2px solid var(--border-strong);
  border-top-color: var(--focus);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

.spinner.small { width: 10px; height: 10px; }

@keyframes spin { to { transform: rotate(360deg); } }

/* ---- 操作栏 ---- */
.action-bar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 6px 8px;
}

.act-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: calc(24px * var(--font-scale, 1));
  height: calc(22px * var(--font-scale, 1));
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  cursor: pointer;
  padding: 0;
  transition: background 0.15s, border-color 0.15s;
}

.act-btn:hover {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.act-btn svg {
  width: calc(13px * var(--font-scale, 1));
  height: calc(13px * var(--font-scale, 1));
}

.act-btn.run {
  background: var(--success);
  border-color: var(--success);
  color: var(--success-fg);
}

.act-btn.run:hover {
  background: var(--success-hover);
  border-color: var(--success-hover);
}

/* ---- 历史下拉 ---- */
.history-pop {
  margin: 0 8px 6px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-panel);
  max-height: 200px;
  overflow: auto;
}

.history-pop-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
  border-bottom: 1px solid var(--border);
}

.link-btn {
  border: none;
  background: none;
  color: var(--focus);
  cursor: pointer;
  font-size: calc(11px * var(--font-scale, 1));
  padding: 0;
}

.history-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  cursor: pointer;
}

.history-row:hover { background: var(--bg-hover); }
.history-row.pinned { background: var(--focus-bg); }

.history-time { font-size: calc(11px * var(--font-scale, 1)); color: var(--fg-secondary); }

.history-elapsed {
  margin-left: auto;
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
}

.history-elapsed.err { color: var(--danger); }

/* ---- 弹窗 ---- */
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
  padding: 20px 24px;
  width: 480px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  overflow: auto;
  box-shadow: 0 8px 24px var(--shadow);
}

.modal.narrow { width: 380px; }

.modal h3 {
  font-size: calc(15px * var(--font-scale, 1));
  margin: 0 0 12px 0;
  color: var(--fg);
}

.modal-text {
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg-secondary);
  line-height: 1.6;
  margin: 0 0 4px 0;
}

.field-label {
  display: block;
  font-size: calc(12px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg-muted);
  margin: 10px 0 4px 0;
}

.field-input {
  width: 100%;
  box-sizing: border-box;
  padding: 6px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: calc(13px * var(--font-scale, 1));
  outline: none;
}

.field-input:focus { border-color: var(--focus); }

.field-hint {
  margin: 4px 0 0 0;
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
  line-height: 1.5;
}

.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg);
  cursor: pointer;
}

.check-row input { accent-color: var(--accent); cursor: pointer; }

/* ---- 调试信息 ---- */
.info-table {
  width: 100%;
  border-collapse: collapse;
  font-size: calc(12px * var(--font-scale, 1));
}

.info-table th {
  text-align: left;
  vertical-align: top;
  width: 110px;
  padding: 4px 8px 4px 0;
  color: var(--fg-muted);
  font-weight: 600;
  white-space: nowrap;
}

.info-table td {
  padding: 4px 0;
  color: var(--fg);
  word-break: break-all;
}

.info-table td.mono { font-family: var(--font-editor); }
.info-table td.err { color: var(--danger); }

.source-view {
  margin: 0;
  padding: 8px;
  background: var(--bg-input-deep);
  border: 1px solid var(--border);
  border-radius: 6px;
  max-height: 200px;
  overflow: auto;
  font-family: var(--font-editor);
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg);
  white-space: pre-wrap;
  word-break: break-word;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.modal-actions button {
  padding: 5px 14px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  cursor: pointer;
  font-size: calc(13px * var(--font-scale, 1));
  transition: background 0.15s, border-color 0.15s;
}

.btn-cancel:hover {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.btn-save {
  background: var(--success);
  color: var(--success-fg);
  border-color: var(--success);
}

.btn-save:hover {
  background: var(--success-hover);
  border-color: var(--success-hover);
}
</style>
