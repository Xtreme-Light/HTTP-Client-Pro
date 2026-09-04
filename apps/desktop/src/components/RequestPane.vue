<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useRequestStore } from '../stores/request';
import { useRunCurrent } from '../composables/useRunCurrent';
import type { HeaderLine } from '../lib/parse';

const requestStore = useRequestStore();
const { run } = useRunCurrent();

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS', 'CONNECT', 'TRACE'];

const activeTab = ref<'params' | 'headers' | 'body'>('headers');

const block = computed(() => requestStore.currentBlock);

// 本地编辑状态
const localMethod = ref('');
const localTarget = ref('');
const localHeaders = ref<HeaderLine[]>([]);
const localBody = ref('');

// 同步 store → 本地
watch(block, (b) => {
  if (!b) return;
  localMethod.value = b.method;
  localTarget.value = b.target;
  localHeaders.value = b.headers.map(h => ({ ...h }));
  localBody.value = b.body;
}, { immediate: true });

function methodClass(m: string): string {
  if (['GET', 'HEAD', 'OPTIONS'].includes(m)) return 'method-safe';
  if (['POST', 'PUT', 'PATCH'].includes(m)) return 'method-write';
  if (['DELETE', 'CONNECT', 'TRACE'].includes(m)) return 'method-danger';
  return 'method-unknown';
}

function applyMethodTarget() {
  requestStore.updateRequestLine(localMethod.value, localTarget.value);
}

function applyHeaders() {
  requestStore.updateHeaders(localHeaders.value.filter(h => h.name.trim()));
}

function applyBody() {
  requestStore.updateBody(localBody.value);
}

function addHeader() {
  localHeaders.value.push({ name: '', value: '', lineIndex: 0 });
}

function removeHeader(idx: number) {
  localHeaders.value.splice(idx, 1);
  applyHeaders();
}

function onMethodChange() {
  applyMethodTarget();
}

function onTargetBlur() {
  applyMethodTarget();
}

function onHeaderBlur() {
  applyHeaders();
}

function onBodyBlur() {
  applyBody();
}

// 查询参数解析
const queryParams = computed(() => {
  const target = block.value?.target ?? '';
  try {
    const url = new URL(target.startsWith('http') ? target : `http://dummy${target}`);
    return Array.from(url.searchParams.entries()).map(([name, value]) => ({ name, value }));
  } catch {
    return [];
  }
});
</script>

<template>
  <div class="request-pane" v-if="block">
    <!-- Method + Target -->
    <div class="request-line">
      <select
        v-model="localMethod"
        @change="onMethodChange"
        :class="['method-select', methodClass(localMethod)]"
      >
        <option v-for="m in HTTP_METHODS" :key="m" :value="m">{{ m }}</option>
      </select>
      <input
        v-model="localTarget"
        @blur="onTargetBlur"
        @keydown.enter="onTargetBlur"
        class="target-input"
        placeholder="https://api.example.com/path"
      />
      <button class="run-btn" @click="run()">
        ▶ Send
      </button>
    </div>

    <!-- Block name -->
    <div v-if="block.name" class="block-name">
      {{ block.name }}
    </div>

    <!-- Tabs -->
    <div class="tabs">
      <button
        :class="['tab', { active: activeTab === 'headers' }]"
        @click="activeTab = 'headers'"
      >
        Headers ({{ localHeaders.filter(h => h.name.trim()).length }})
      </button>
      <button
        :class="['tab', { active: activeTab === 'body' }]"
        @click="activeTab = 'body'"
      >
        Body
      </button>
      <button
        :class="['tab', { active: activeTab === 'params' }]"
        @click="activeTab = 'params'"
      >
        Params
      </button>
    </div>

    <!-- Headers tab -->
    <div v-show="activeTab === 'headers'" class="tab-content">
      <div class="header-list">
        <div v-for="(h, idx) in localHeaders" :key="idx" class="header-row">
          <input
            v-model="h.name"
            @blur="onHeaderBlur"
            class="header-name-input"
            placeholder="Header name"
          />
          <input
            v-model="h.value"
            @blur="onHeaderBlur"
            class="header-value-input"
            placeholder="Header value"
          />
          <button class="remove-btn" @click="removeHeader(idx)" title="Remove">×</button>
        </div>
      </div>
      <button class="add-btn" @click="addHeader">+ Add header</button>
    </div>

    <!-- Body tab -->
    <div v-show="activeTab === 'body'" class="tab-content">
      <textarea
        v-model="localBody"
        @blur="onBodyBlur"
        class="body-editor"
        placeholder="Request body (JSON, text, etc.)"
        spellcheck="false"
      />
      <div v-if="block.handler" class="handler-info">
        <span class="handler-label">Handler:</span>
        <code>{{ block.handler }}</code>
      </div>
      <div v-if="block.responseRef" class="handler-info">
        <span class="handler-label">Response ref:</span>
        <code>{{ block.responseRef }}</code>
      </div>
    </div>

    <!-- Params tab -->
    <div v-show="activeTab === 'params'" class="tab-content">
      <div class="params-view">
        <p class="hint">Query parameters are parsed from the URL target.</p>
        <div v-if="queryParams.length" class="param-list">
          <div v-for="p in queryParams" :key="p.name" class="param-row">
            <span class="param-name">{{ p.name }}</span>
            <span class="param-value">{{ p.value }}</span>
          </div>
        </div>
        <p v-else class="hint">No query parameters found.</p>
      </div>
    </div>
  </div>

  <div v-else class="empty-state">
    No request block selected.
  </div>
</template>

<style scoped>
.request-pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--bg-base);
}

.request-line {
  display: flex;
  gap: 4px;
  padding: 8px;
  border-bottom: 1px solid var(--border);
}

.method-select {
  padding: 4px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.method-safe { color: var(--chip-safe-fg); }
.method-write { color: var(--chip-write-fg); }
.method-danger { color: var(--chip-danger-fg); }

.target-input {
  flex: 1;
  padding: 4px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: 13px;
  font-family: ui-monospace, monospace;
  outline: none;
}

.target-input:focus {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.run-btn {
  padding: 4px 16px;
  border: 1px solid var(--success);
  border-radius: 4px;
  background: var(--success);
  color: var(--success-fg);
  font-weight: 600;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s;
}

.run-btn:hover { background: var(--success-hover); }

.block-name {
  padding: 4px 12px;
  font-size: 12px;
  color: var(--syn-keyword);
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
}

.tabs {
  display: flex;
  border-bottom: 1px solid var(--border);
  background: var(--bg-panel);
}

.tab {
  padding: 6px 12px;
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

.header-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.header-row {
  display: flex;
  gap: 4px;
  align-items: center;
}

.header-name-input {
  width: 35%;
  padding: 3px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: 12px;
  font-family: ui-monospace, monospace;
}

.header-value-input {
  flex: 1;
  padding: 3px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: 12px;
  font-family: ui-monospace, monospace;
}

.remove-btn {
  padding: 2px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--danger);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
}

.remove-btn:hover { background: var(--danger-bg); }

.add-btn {
  margin-top: 8px;
  padding: 4px 12px;
  border: 1px dashed var(--border-strong);
  border-radius: 4px;
  background: var(--bg-panel);
  color: var(--focus);
  cursor: pointer;
  font-size: 12px;
  width: 100%;
  text-align: center;
}

.add-btn:hover { background: var(--bg-hover); }

.body-editor {
  width: 100%;
  min-height: 120px;
  padding: 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input-deep);
  color: var(--fg);
  font-size: 13px;
  font-family: ui-monospace, monospace;
  resize: vertical;
  outline: none;
}

.body-editor:focus {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.handler-info {
  margin-top: 8px;
  padding: 6px 8px;
  background: var(--bg-panel);
  border-radius: 4px;
  font-size: 12px;
  display: flex;
  gap: 6px;
  align-items: center;
}

.handler-label {
  color: var(--fg-muted);
  font-weight: 600;
}

.handler-info code {
  font-family: ui-monospace, monospace;
  color: var(--syn-url);
  font-size: 12px;
}

.params-view { padding: 4px; }

.hint {
  color: var(--fg-muted);
  font-size: 12px;
  margin-bottom: 8px;
}

.param-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.param-row {
  display: flex;
  gap: 8px;
  padding: 3px 6px;
  background: var(--bg-panel);
  border-radius: 3px;
  font-size: 12px;
  font-family: ui-monospace, monospace;
}

.param-name { color: var(--syn-url); }
.param-value { color: var(--syn-string); }

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--fg-muted);
  font-size: 14px;
}
</style>
