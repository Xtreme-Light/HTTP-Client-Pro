<script setup lang="ts">
import { ref } from 'vue';
import { useWorkspaceStore } from '../stores/workspace';

const workspaceStore = useWorkspaceStore();

// 未保存更改弹窗状态
const unsavedDialog = ref<{ path: string; name: string } | null>(null);

function onTabClick(path: string) {
  workspaceStore.switchTab(path);
}

/** 双击标签栏空白区域 → 立即新建 Untitled.http 标签页 */
function onBarDblClick(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('.editor-tab')) return;
  workspaceStore.createUntitledTab();
}

function onTabClose(path: string) {
  const tab = workspaceStore.getTab(path);
  if (!tab) return;
  // 设置 / 只读示例标签页直接关闭，无需未保存检查
  if (tab.type === 'settings' || tab.type === 'example') {
    workspaceStore.closeTab(path);
    return;
  }
  if (tab.isDirty) {
    unsavedDialog.value = { path, name: tab.name };
  } else {
    workspaceStore.closeTab(path);
  }
}

function onDialogCancel() {
  unsavedDialog.value = null;
}

function onDialogDontSave() {
  if (unsavedDialog.value) {
    workspaceStore.closeTab(unsavedDialog.value.path);
  }
  unsavedDialog.value = null;
}

async function onDialogSave() {
  if (!unsavedDialog.value) return;
  const { path } = unsavedDialog.value;
  try {
    // 未命名标签页由 store 保存到默认工作区并转为正式文件标签（原标签已关闭）
    await workspaceStore.saveTab(path);
  } catch (e) {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
    return;
  }
  if (workspaceStore.getTab(path)) {
    workspaceStore.closeTab(path);
  }
  unsavedDialog.value = null;
}
</script>

<template>
  <div class="editor-tab-bar" title="双击空白处新建 Untitled.http" @dblclick="onBarDblClick">
    <div class="tabs-scroll">
      <div
        v-for="tab in workspaceStore.tabs"
        :key="tab.path"
        class="editor-tab"
        :class="{ active: tab.path === workspaceStore.activeTabPath }"
        :title="tab.type === 'settings' ? '设置' : tab.type === 'example' ? `${tab.name}（只读示例）` : tab.path"
        @click="onTabClick(tab.path)"
      >
        <span class="tab-name">{{ tab.name }}</span>
        <svg v-if="tab.type === 'example'" class="tab-readonly" width="10" height="10" viewBox="0 0 12 12" fill="currentColor" aria-label="只读">
          <path d="M6 1a2.5 2.5 0 0 1 2.5 2.5V5h.5a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h.5V3.5A2.5 2.5 0 0 1 6 1zm1.5 4V3.5a1.5 1.5 0 1 0-3 0V5h3z"/>
        </svg>
        <span v-if="tab.isDirty" class="tab-dirty">●</span>
        <button class="tab-close" title="Close" @click.stop="onTabClose(tab.path)">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="3" y1="3" x2="9" y2="9" />
            <line x1="9" y1="3" x2="3" y2="9" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 未保存更改弹窗 -->
    <div v-if="unsavedDialog" class="modal-overlay" @click="onDialogCancel">
      <div class="modal" @click.stop>
        <h3>未保存的更改</h3>
        <p class="modal-text">[{{ unsavedDialog.name }}]有未保存的更改，关闭后将丢失，是否保存后再关闭？</p>
        <div class="modal-actions">
          <button class="btn-cancel" @click="onDialogCancel">取消</button>
          <button class="btn-dont-save" @click="onDialogDontSave">不保存</button>
          <button class="btn-save" @click="onDialogSave">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.editor-tab-bar {
  display: flex;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  min-height: 30px;
}

.tabs-scroll {
  display: flex;
  overflow-x: auto;
  scrollbar-width: thin;
}

.tabs-scroll::-webkit-scrollbar {
  height: 2px;
}

.tabs-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar);
}

.editor-tab {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  height: 30px;
  cursor: pointer;
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-muted);
  white-space: nowrap;
  border-right: 1px solid var(--border);
  transition: background 0.15s, color 0.15s;
  position: relative;
}

.editor-tab:hover {
  background: var(--bg-chrome);
  color: var(--fg-secondary);
}

.editor-tab.active {
  background: var(--bg-base);
  color: var(--fg-strong);
}

.editor-tab.active::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--accent);
}

.tab-name {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-dirty {
  color: var(--warning);
  font-size: calc(10px * var(--font-scale, 1));
  flex-shrink: 0;
}

.tab-readonly {
  color: var(--fg-muted);
  flex-shrink: 0;
}

.tab-close {
  display: flex;
  align-items: center;
  justify-content: center;
  /* 关闭按钮里的 × 是字形，随字号一起缩放，避免大字号时溢出 */
  width: calc(16px * var(--font-scale, 1));
  height: calc(16px * var(--font-scale, 1));
  border: none;
  background: none;
  color: var(--fg-muted);
  cursor: pointer;
  border-radius: 3px;
  flex-shrink: 0;
  transition: background 0.15s, color 0.15s;
}

.tab-close:hover {
  background: var(--bg-active);
  color: var(--fg-strong);
}

/* 弹窗样式 */
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
  min-width: 400px;
  max-width: 500px;
  box-shadow: 0 8px 24px var(--shadow);
}

.modal h3 {
  font-size: calc(14px * var(--font-scale, 1));
  margin: 0 0 12px 0;
  color: var(--fg);
}

.modal-text {
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg-secondary);
  line-height: 1.6;
  margin: 0 0 16px 0;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
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

.btn-dont-save {
  background: transparent;
  color: var(--danger);
  border-color: var(--danger-border);
}

.btn-dont-save:hover {
  background: var(--danger-bg);
  border-color: var(--danger);
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
