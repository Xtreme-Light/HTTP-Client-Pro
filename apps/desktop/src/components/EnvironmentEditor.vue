<script setup lang="ts">
import { ref, computed } from 'vue';
import { useEnvironmentStore } from '../stores/environment';

const envStore = useEnvironmentStore();

const showPanel = ref(false);
const newVarName = ref('');
const newVarValue = ref('');
const newEnvName = ref('');

const vars = computed(() => {
  if (!envStore.current) return [];
  const envVars = envStore.vars;
  return Object.entries(envVars).map(([key, value]) => ({ key, value }));
});

function addVar() {
  if (!newVarName.value.trim()) return;
  envStore.setVar(newVarName.value.trim(), newVarValue.value);
  newVarName.value = '';
  newVarValue.value = '';
}

function removeVar(key: string) {
  envStore.deleteVar(key);
}

function addEnv() {
  if (!newEnvName.value.trim()) return;
  envStore.addEnv(newEnvName.value.trim());
  envStore.setCurrent(newEnvName.value.trim());
  newEnvName.value = '';
}

function togglePanel() {
  showPanel.value = !showPanel.value;
}
</script>

<template>
  <div class="env-editor">
    <button class="env-toggle-btn" @click="togglePanel">
      ⚙ {{ envStore.current ?? 'No env' }}
    </button>

    <div v-if="showPanel" class="env-panel">
      <!-- 环境切换 -->
      <div class="env-section">
        <div class="section-label">Environments</div>
        <div class="env-tabs">
          <button
            v-for="name in envStore.envNames"
            :key="name"
            :class="['env-tab', { active: name === envStore.current }]"
            @click="envStore.setCurrent(name)"
          >
            {{ name }}
          </button>
        </div>
        <div class="add-env-row">
          <input
            v-model="newEnvName"
            @keydown.enter="addEnv"
            class="env-name-input"
            placeholder="New env name"
          />
          <button class="add-env-btn" @click="addEnv">+</button>
        </div>
      </div>

      <!-- 变量编辑 -->
      <div class="env-section">
        <div class="section-label">Variables ({{ vars.length }})</div>
        <div class="var-list">
          <div v-for="v in vars" :key="v.key" class="var-row">
            <span class="var-key">{{ v.key }}</span>
            <input
              :value="v.value"
              @input="envStore.setVar(v.key, ($event.target as HTMLInputElement).value)"
              class="var-value-input"
            />
            <button class="remove-var-btn" @click="removeVar(v.key)">×</button>
          </div>
        </div>

        <!-- 添加新变量 -->
        <div class="add-var-row">
          <input
            v-model="newVarName"
            @keydown.enter="addVar"
            class="var-name-input"
            placeholder="Variable name"
          />
          <input
            v-model="newVarValue"
            @keydown.enter="addVar"
            class="var-value-input"
            placeholder="Value"
          />
          <button class="add-var-btn" @click="addVar">Add</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.env-editor {
  position: relative;
}

.env-toggle-btn {
  padding: 2px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg-secondary);
  font-size: 12px;
  cursor: pointer;
}

.env-toggle-btn:hover { background: var(--bg-button); }

.env-panel {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 4px;
  width: 360px;
  max-height: 400px;
  overflow: auto;
  background: var(--bg-chrome);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 4px 12px var(--shadow);
  z-index: 100;
  padding: 12px;
}

.env-section {
  margin-bottom: 12px;
}

.env-section:last-child { margin-bottom: 0; }

.section-label {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--fg-muted);
  margin-bottom: 6px;
  letter-spacing: 0.5px;
}

.env-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}

.env-tab {
  padding: 2px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  background: var(--bg-input);
  font-size: 12px;
  cursor: pointer;
  color: var(--fg-secondary);
}

.env-tab.active {
  background: var(--focus);
  color: var(--fg-strong);
  border-color: var(--focus);
}

.add-env-row {
  display: flex;
  gap: 4px;
}

.env-name-input {
  flex: 1;
  padding: 2px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  font-size: 12px;
  background: var(--bg-input-deep);
  color: var(--fg);
}

.add-env-btn {
  padding: 2px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  cursor: pointer;
  font-size: 14px;
  color: var(--focus);
}

.var-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.var-row {
  display: flex;
  align-items: center;
  gap: 4px;
}

.var-key {
  width: 100px;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  color: var(--syn-keyword);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}

.var-value-input {
  flex: 1;
  padding: 2px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  background: var(--bg-input-deep);
  color: var(--fg);
}

.remove-var-btn {
  padding: 2px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--danger);
  cursor: pointer;
  font-size: 12px;
}

.remove-var-btn:hover { background: var(--danger-bg); }

.add-var-row {
  display: flex;
  gap: 4px;
  align-items: center;
}

.var-name-input {
  width: 100px;
  padding: 2px 6px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
  font-size: 12px;
  flex-shrink: 0;
  background: var(--bg-input-deep);
  color: var(--fg);
}

.add-var-btn {
  padding: 2px 12px;
  border: 1px solid var(--success);
  border-radius: 3px;
  background: var(--success);
  color: var(--success-fg);
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}

.add-var-btn:hover { background: var(--success-hover); }
</style>
