<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useWorkspaceStore, isUntitledPath } from '../stores/workspace';
import { useSettingsStore } from '../stores/settings';
import { getFs } from '../lib/backend/fs';

const workspaceStore = useWorkspaceStore();
const settings = useSettingsStore();

/* ================= 布局（位置 / 单行多行） ================= */

const barClass = computed(() => [
  `pos-${settings.tabPosition}`,
  `layout-${settings.tabLayout}`,
]);

/* ================= 未保存更改弹窗（单个 / 批量共用） ================= */

type DialogChoice = 'save' | 'discard' | 'cancel';

const unsavedDialog = ref<{ path: string; name: string } | null>(null);
let dialogResolve: ((choice: DialogChoice) => void) | null = null;

/** 弹出未保存询问，返回用户选择（Promise 化以支持批量关闭时串行询问） */
function askUnsaved(path: string, name: string): Promise<DialogChoice> {
  return new Promise((resolve) => {
    dialogResolve = resolve;
    unsavedDialog.value = { path, name };
  });
}

function finishDialog(choice: DialogChoice) {
  unsavedDialog.value = null;
  const resolve = dialogResolve;
  dialogResolve = null;
  if (resolve) resolve(choice);
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
  finishDialog('save');
}

/**
 * 关闭单个标签页（含未保存询问）。返回是否实际关闭。
 * 设置 / 只读示例标签页直接关闭，无需未保存检查。
 */
async function closeTabWithConfirm(path: string): Promise<boolean> {
  const tab = workspaceStore.getTab(path);
  if (!tab) return false;
  if (tab.type === 'settings' || tab.type === 'example' || !tab.isDirty) {
    workspaceStore.closeTab(path);
    return true;
  }
  const choice = await askUnsaved(path, tab.name);
  if (choice === 'cancel') return false;
  if (choice === 'save' && !workspaceStore.getTab(path)) {
    // 未命名标签保存时已原地转正（原路径已关闭）
    return true;
  }
  workspaceStore.closeTab(path);
  return true;
}

/** 批量关闭：逐个询问未保存标签，任一处取消则中止后续 */
async function closeTabsSerially(paths: string[]) {
  for (const p of paths) {
    const closed = await closeTabWithConfirm(p);
    if (!closed && workspaceStore.getTab(p)) return; // 用户取消
  }
}

function onTabClose(path: string) {
  void closeTabWithConfirm(path);
}

/* ================= 容量淘汰弹窗（全部标签未保存时由 store 触发） ================= */

const evictDialog = ref<{ path: string; name: string } | null>(null);

watch(
  () => workspaceStore.pendingEviction,
  (p) => { evictDialog.value = p ? { ...p } : null; },
);

async function onEvictSave() {
  if (!evictDialog.value) return;
  try {
    await workspaceStore.saveTab(evictDialog.value.path);
  } catch (e) {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
    return;
  }
  evictDialog.value = null;
  workspaceStore.resolveEviction('save');
}

function onEvictDontSave() {
  evictDialog.value = null;
  workspaceStore.resolveEviction('discard');
}

function onEvictCancel() {
  evictDialog.value = null;
  workspaceStore.resolveEviction('cancel');
}

/* ================= 右键菜单 ================= */

interface TabCtx { kind: 'tab'; x: number; y: number; path: string }
interface BarCtx { kind: 'bar'; x: number; y: number }
type CtxState = TabCtx | BarCtx;

const ctx = ref<CtxState | null>(null);

/** 防止菜单超出视口（估算尺寸，出现后如仍有偏差由浏览器裁剪） */
function clampPos(x: number, y: number, w = 190, h = 230) {
  return {
    x: Math.max(0, Math.min(x, window.innerWidth - w - 4)),
    y: Math.max(0, Math.min(y, window.innerHeight - h - 4)),
  };
}

function onTabContextMenu(e: MouseEvent, path: string) {
  e.preventDefault();
  e.stopPropagation();
  const { x, y } = clampPos(e.clientX, e.clientY);
  ctx.value = { kind: 'tab', x, y, path };
}

function onBarContextMenu(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('.editor-tab')) return;
  e.preventDefault();
  const { x, y } = clampPos(e.clientX, e.clientY, 170, 90);
  ctx.value = { kind: 'bar', x, y };
}

function closeCtx() {
  ctx.value = null;
}

function onDocMouseDown(e: MouseEvent) {
  if (ctx.value && !(e.target as HTMLElement).closest('.ctx-menu')) closeCtx();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') closeCtx();
}

onMounted(() => {
  document.addEventListener('mousedown', onDocMouseDown, true);
  document.addEventListener('keydown', onKeydown);
});

onUnmounted(() => {
  document.removeEventListener('mousedown', onDocMouseDown, true);
  document.removeEventListener('keydown', onKeydown);
});

/** 当前右键的标签页对象 */
const ctxTab = computed(() =>
  ctx.value?.kind === 'tab' ? workspaceStore.getTab(ctx.value.path) : undefined,
);
const ctxIsFile = computed(() => ctxTab.value?.type === 'file');
/** 「打开所在位置」仅对磁盘上真实存在的文件标签可用 */
const ctxCanReveal = computed(() =>
  !!ctxTab.value && ctxIsFile.value && !isUntitledPath(ctxTab.value.path) && !!getFs(),
);

function onCtxSave() {
  if (ctx.value?.kind !== 'tab') return;
  const { path } = ctx.value;
  closeCtx();
  workspaceStore.switchTab(path);
  workspaceStore.saveTab(path).catch((e) => {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
  });
}

function onCtxSaveAs() {
  if (ctx.value?.kind !== 'tab') return;
  const { path } = ctx.value;
  closeCtx();
  // 另存为统一由 App.vue 处理（原生保存对话框），指定来源标签页
  window.dispatchEvent(new CustomEvent('app:save-as', { detail: { path } }));
}

function onCtxClose() {
  if (ctx.value?.kind !== 'tab') return;
  const { path } = ctx.value;
  closeCtx();
  void closeTabWithConfirm(path);
}

function onCtxCloseOthers() {
  if (ctx.value?.kind !== 'tab') return;
  const { path } = ctx.value;
  closeCtx();
  const others = workspaceStore.tabs
    .filter((t) => t.path !== path && t.type !== 'settings')
    .map((t) => t.path);
  void closeTabsSerially(others);
}

function onCtxCloseAll() {
  closeCtx();
  const all = workspaceStore.tabs.filter((t) => t.type !== 'settings').map((t) => t.path);
  void closeTabsSerially(all);
}

function onCtxReveal() {
  if (ctx.value?.kind !== 'tab' || !ctxCanReveal.value) return;
  const { path } = ctx.value;
  closeCtx();
  const fs = getFs();
  if (!fs) return;
  fs.revealInDir(path).catch((e) => {
    alert(`打开所在位置失败：${e instanceof Error ? e.message : String(e)}`);
  });
}

function onCtxNewTab() {
  closeCtx();
  void workspaceStore.createUntitledTab();
}

/** 「标签页设置…」— 打开设置标签页并直达「标签页」配置区 */
function onCtxTabSettings() {
  closeCtx();
  workspaceStore.openSettings();
  // 设置页可能刚被创建，延迟一拍等 SettingsPane 挂载并注册监听后再派发
  setTimeout(() => {
    window.dispatchEvent(new CustomEvent('app:open-tab-settings'));
  }, 0);
}

/* ================= 其他交互 ================= */

function onTabClick(path: string) {
  workspaceStore.switchTab(path);
}

/** 中键点击标签 → 关闭（IDE 惯例） */
function onTabAuxClick(e: MouseEvent, path: string) {
  if (e.button !== 1) return;
  e.preventDefault();
  void closeTabWithConfirm(path);
}

/** 双击标签栏空白区域 → 立即新建 Untitled.http 标签页 */
function onBarDblClick(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('.editor-tab')) return;
  void workspaceStore.createUntitledTab();
}
</script>

<template>
  <div
    class="editor-tab-bar"
    :class="barClass"
    title="双击空白处新建 Untitled.http"
    @dblclick="onBarDblClick"
    @contextmenu="onBarContextMenu"
  >
    <div class="tabs-scroll">
      <div
        v-for="tab in workspaceStore.tabs"
        :key="tab.path"
        class="editor-tab"
        :class="{ active: tab.path === workspaceStore.activeTabPath }"
        :title="tab.type === 'settings' ? '设置' : tab.type === 'example' ? `${tab.name}（只读示例）` : tab.path"
        @click="onTabClick(tab.path)"
        @auxclick="onTabAuxClick($event, tab.path)"
        @contextmenu="onTabContextMenu($event, tab.path)"
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

    <!-- 标签页右键菜单 -->
    <div
      v-if="ctx && ctx.kind === 'tab'"
      class="ctx-menu"
      :style="{ left: ctx.x + 'px', top: ctx.y + 'px' }"
      @click.stop
      @contextmenu.prevent
    >
      <button v-if="ctxIsFile" class="menu-item" @click="onCtxSave">保存</button>
      <button v-if="ctxIsFile" class="menu-item" @click="onCtxSaveAs">另存为…</button>
      <div v-if="ctxIsFile" class="menu-sep" />
      <button class="menu-item" @click="onCtxClose">关闭</button>
      <button class="menu-item" @click="onCtxCloseAll">关闭全部</button>
      <button
        class="menu-item"
        :disabled="workspaceStore.tabs.filter((t) => t.type !== 'settings').length <= 1"
        @click="onCtxCloseOthers"
      >
        关闭其他
      </button>
      <div v-if="ctxIsFile" class="menu-sep" />
      <button
        v-if="ctxIsFile"
        class="menu-item"
        :disabled="!ctxCanReveal"
        :title="ctxCanReveal ? '' : '仅磁盘上已保存的文件支持'"
        @click="onCtxReveal"
      >
        打开所在位置
      </button>
    </div>

    <!-- 标签栏空白处右键菜单 -->
    <div
      v-if="ctx && ctx.kind === 'bar'"
      class="ctx-menu"
      :style="{ left: ctx.x + 'px', top: ctx.y + 'px' }"
      @click.stop
      @contextmenu.prevent
    >
      <button class="menu-item" @click="onCtxNewTab">新建标签页</button>
      <button class="menu-item" @click="onCtxTabSettings">标签页设置…</button>
    </div>

    <!-- 未保存更改弹窗 -->
    <div v-if="unsavedDialog" class="modal-overlay" @click="finishDialog('cancel')">
      <div class="modal" @click.stop>
        <h3>未保存的更改</h3>
        <p class="modal-text">[{{ unsavedDialog.name }}]有未保存的更改，关闭后将丢失，是否保存后再关闭？</p>
        <div class="modal-actions">
          <button class="btn-cancel" @click="finishDialog('cancel')">取消</button>
          <button class="btn-dont-save" @click="finishDialog('discard')">不保存</button>
          <button class="btn-save" @click="onDialogSave">保存</button>
        </div>
      </div>
    </div>

    <!-- 容量淘汰弹窗：超出最大标签数且全部未保存 -->
    <div v-if="evictDialog" class="modal-overlay" @click="onEvictCancel">
      <div class="modal" @click.stop>
        <h3>标签页数量已达上限</h3>
        <p class="modal-text">
          已达最大标签页数量（{{ settings.maxTabs }}），且所有标签页均有未保存的修改。
          是否保存 [{{ evictDialog.name }}] 后将其关闭，以便打开新标签页？
        </p>
        <div class="modal-actions">
          <button class="btn-cancel" @click="onEvictCancel">取消</button>
          <button class="btn-dont-save" @click="onEvictDontSave">不保存并关闭</button>
          <button class="btn-save" @click="onEvictSave">保存并关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ================= 标签栏容器：位置（头部 / 左侧 / 右侧） ================= */
.editor-tab-bar {
  display: flex;
  background: var(--bg-panel);
  flex-shrink: 0;
}

.editor-tab-bar.pos-top {
  flex-direction: row;
  border-bottom: 1px solid var(--border);
  min-height: 30px;
}

.editor-tab-bar.pos-left,
.editor-tab-bar.pos-right {
  flex-direction: column;
  width: 180px;
  min-width: 180px;
  overflow: hidden;
}

.editor-tab-bar.pos-left {
  border-right: 1px solid var(--border);
}

.editor-tab-bar.pos-right {
  border-left: 1px solid var(--border);
}

/* ================= 标签滚动容器：单行 / 多行 ================= */
.tabs-scroll {
  display: flex;
  min-width: 0;
  scrollbar-width: thin;
}

.tabs-scroll::-webkit-scrollbar {
  height: 2px;
  width: 2px;
}

.tabs-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar);
}

/* 头部 + 单行：横向滚动（layout-* 类挂在根元素上） */
.pos-top.layout-single .tabs-scroll {
  flex-direction: row;
  flex-wrap: nowrap;
  overflow-x: auto;
}

/* 头部 + 多行：自动换行，超过约 3 行后纵向滚动 */
.pos-top.layout-multi .tabs-scroll {
  flex-direction: row;
  flex-wrap: wrap;
  overflow-y: auto;
  max-height: 96px;
}

/* 左右侧：纵向排列，始终单列 */
.pos-left .tabs-scroll,
.pos-right .tabs-scroll {
  flex-direction: column;
  flex: 1;
  overflow-y: auto;
}

/* ================= 单个标签 ================= */
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
  transition: background 0.15s, color 0.15s;
  position: relative;
  flex-shrink: 0;
}

.pos-top .editor-tab {
  border-right: 1px solid var(--border);
}

.pos-left .editor-tab,
.pos-right .editor-tab {
  width: 100%;
  box-sizing: border-box;
  border-bottom: 1px solid var(--border);
}

.editor-tab:hover {
  background: var(--bg-chrome);
  color: var(--fg-secondary);
}

.editor-tab.active {
  background: var(--bg-base);
  color: var(--fg-strong);
}

/* 激活指示条：随标签栏位置贴靠编辑器一侧 */
.editor-tab.active::after {
  content: '';
  position: absolute;
  background: var(--accent);
}

.pos-top .editor-tab.active::after {
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
}

.pos-left .editor-tab.active::after {
  top: 0;
  bottom: 0;
  right: 0;
  width: 2px;
}

.pos-right .editor-tab.active::after {
  top: 0;
  bottom: 0;
  left: 0;
  width: 2px;
}

.tab-name {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pos-left .tab-name,
.pos-right .tab-name {
  flex: 1;
  max-width: none;
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

/* ================= 右键菜单 ================= */
.ctx-menu {
  position: fixed;
  background: var(--bg-chrome);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 4px 12px var(--shadow);
  padding: 4px 0;
  z-index: 10000;
  min-width: 150px;
}

.menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 4px 12px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg);
}

.menu-item:hover:not(:disabled) {
  background: var(--bg-hover);
}

.menu-item:disabled {
  color: var(--fg-muted);
  cursor: default;
  opacity: 0.6;
}

.menu-sep {
  height: 1px;
  background: var(--border);
  margin: 4px 0;
}

/* ================= 弹窗样式 ================= */
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
