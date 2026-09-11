<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { Splitpanes, Pane } from 'splitpanes';
import 'splitpanes/dist/splitpanes.css';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useRequestStore } from './stores/request';
import { useHistoryStore } from './stores/history';
import { useEnvironmentStore } from './stores/environment';
import { useWorkspaceStore } from './stores/workspace';
import { useSettingsStore } from './stores/settings';
import { setBackendAdapter, useRunCurrent } from './composables/useRunCurrent';
import { detectAdapter } from './lib/backend';
import { getFs } from './lib/backend/fs';
import { eventMatchesBinding, getKeymapScheme, type ShortcutAction } from './lib/keymaps';
import EditorPane from './components/EditorPane.vue';
import EditorTabBar from './components/EditorTabBar.vue';
import SettingsPane from './components/SettingsPane.vue';
import ResponsePane from './components/ResponsePane.vue';
import HistoryPanel from './components/HistoryPanel.vue';
import WorkspaceSidebar from './components/WorkspaceSidebar.vue';
import TitleBar from './components/TitleBar.vue';

const requestStore = useRequestStore();
const historyStore = useHistoryStore();
const envStore = useEnvironmentStore();
const workspaceStore = useWorkspaceStore();
const settings = useSettingsStore();

const showSidebar = ref(true);
const { run } = useRunCurrent();

/** 未保存的临时标签页路径格式 */
const UNTITLED_RE = /^untitled-\d+$/;

onMounted(async () => {
  window.addEventListener('keydown', onGlobalKeydown);
  // 浏览器环境的兜底：Tauri 关窗不一定触发 beforeunload，另见 registerCloseHook
  window.addEventListener('beforeunload', onBeforeUnload);
  await registerCloseHook();
  try {
    const adapter = detectAdapter();
    setBackendAdapter(adapter);
  } catch (e) {
    console.error('Failed to detect backend adapter:', e);
  }
  historyStore.load();
  envStore.load();
  workspaceStore.load();
  requestStore.setSource(requestStore.source);

  const fs = getFs();
  // 优先恢复上次退出现场；恢复失败（首次启动 / 数据损坏）走默认文件初始化
  const restored = workspaceStore.restoreSession();
  if (fs && restored) await refreshCleanTabsFromDisk();

  if (fs && !restored && !workspaceStore.currentFilePath) {
    try {
      const defaultPath = await fs.getDefaultWorkspace();
      if (workspaceStore.roots.length === 0) {
        workspaceStore.addRoot(defaultPath, 'Default');
      }
      const defaultFile = `${defaultPath}/requests.http`;
      let existingContent: string | null = null;
      try {
        existingContent = await fs.readFile(defaultFile);
      } catch { /* file not found */ }
      if (existingContent === null) {
        await fs.writeFile(defaultFile, requestStore.source);
        workspaceStore.notifyFsChange();
        workspaceStore.openFile(defaultFile, 'requests.http', requestStore.source);
      } else if (existingContent.length > 0) {
        workspaceStore.openFile(defaultFile, 'requests.http', existingContent);
      } else {
        workspaceStore.openFile(defaultFile, 'requests.http', requestStore.source);
      }
    } catch (e) {
      console.error('Failed to init default file:', e);
    }
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
  window.removeEventListener('beforeunload', onBeforeUnload);
  unlistenClose?.();
  unlistenClose = null;
  cancelPendingSessionSave();
});

/* ================= 会话持久化 ================= */

/** 现场变化后的自动保存延迟（毫秒） */
const SESSION_SAVE_DELAY = 300;

let sessionTimer: ReturnType<typeof setTimeout> | null = null;
let unlistenClose: (() => void) | null = null;

function cancelPendingSessionSave() {
  if (sessionTimer !== null) {
    clearTimeout(sessionTimer);
    sessionTimer = null;
  }
}

/** 立即保存现场，并丢弃待触发的自动保存 */
function flushSession() {
  cancelPendingSessionSave();
  workspaceStore.saveSession();
}

/** 退出前保存现场（浏览器 / 开发环境兜底） */
function onBeforeUnload() {
  flushSession();
}

// Tauri 关窗（含 Alt+F4、任务栏关闭）不一定触发 beforeunload，
// 因此现场随用随存：标签页或激活状态变化后防抖写入 localStorage。
watch(
  () => [workspaceStore.tabs, workspaceStore.activeTabPath] as const,
  () => {
    cancelPendingSessionSave();
    sessionTimer = setTimeout(() => {
      sessionTimer = null;
      workspaceStore.saveSession();
    }, SESSION_SAVE_DELAY);
  },
  { deep: true },
);

/**
 * 拦截 Tauri 的关闭请求：先同步落盘现场，再销毁窗口。
 * 非 Tauri 环境（浏览器调试）注册失败时静默忽略，由 beforeunload 兜底。
 */
async function registerCloseHook() {
  try {
    const win = getCurrentWindow();
    unlistenClose = await win.onCloseRequested(async (event) => {
      event.preventDefault();
      flushSession();
      await win.destroy();
    });
  } catch { /* 非 Tauri 环境 */ }
}

/**
 * 恢复现场后，已保存（干净）的文件标签从磁盘重读，采用外部修改；
 * 有未保存修改的标签保留会话中的编辑内容。
 */
async function refreshCleanTabsFromDisk() {
  const fs = getFs();
  if (!fs) return;
  for (const tab of workspaceStore.tabs) {
    if (tab.type !== 'file' || tab.isDirty) continue;
    try {
      tab.content = await fs.readFile(tab.path);
    } catch { /* 文件已删除 / 未命名标签 → 保留会话中的内容 */ }
  }
  const active = workspaceStore.getTab(workspaceStore.activeTabPath ?? '');
  if (active?.type === 'file') requestStore.setSource(active.content);
}

/* ================= 文件操作 ================= */

/** 待写入磁盘的内容 — 取标签页的最新内容（编辑器回写 store 有防抖延迟） */
function contentToSave(path: string): string {
  return workspaceStore.getTab(path)?.content ?? requestStore.source;
}

async function saveFile() {
  if (!workspaceStore.canSave) return;
  const path = workspaceStore.currentFilePath;
  // 未命名的临时标签页 → 走「另存为」流程
  if (path && UNTITLED_RE.test(path)) {
    await saveAs();
    return;
  }
  const fs = getFs();
  if (!fs || !path) return;
  try {
    await fs.writeFile(path, contentToSave(path));
    workspaceStore.markClean();
    workspaceStore.notifyFsChange();
  } catch (e) {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function saveAs() {
  const fs = getFs();
  if (!fs) return;
  const prevPath = workspaceStore.currentFilePath;
  const canUsePrev = prevPath && workspaceStore.canSave && !UNTITLED_RE.test(prevPath);
  const fallback = workspaceStore.roots.length > 0
    ? `${workspaceStore.roots[0].path}/requests.http`
    : '~/requests.http';
  const name = prompt('Save as (full path):', canUsePrev ? prevPath : fallback);
  if (!name) return;
  const content = prevPath ? contentToSave(prevPath) : requestStore.source;
  try {
    await fs.writeFile(name, content);
    const parts = name.split('/');
    workspaceStore.openFile(name, parts[parts.length - 1] || name, content);
    // 「另存为」成功后关闭来源的未命名标签页
    if (prevPath && UNTITLED_RE.test(prevPath)) {
      workspaceStore.closeTab(prevPath);
    }
    // 目标标签页内容与已写入内容一致时才标记为已保存
    if (workspaceStore.getTab(name)?.content === content) {
      workspaceStore.markTabClean(name);
    }
    workspaceStore.notifyFsChange();
  } catch (e) {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** 新建文件：创建一个未命名标签页（保存时转入「另存为」流程） */
function newFile() {
  let n = 1;
  while (workspaceStore.getTab(`untitled-${n}`)) n++;
  const path = `untitled-${n}`;
  workspaceStore.openFile(path, `${path}.http`, '');
}

/** 打开文件：弹出文件选择对话框并载入标签页 */
async function openFilePicker() {
  const fs = getFs();
  if (!fs) return;
  try {
    const path = await fs.pickFile();
    if (!path) return;
    const content = await fs.readFile(path);
    const name = path.split('/').pop() || path;
    workspaceStore.openFile(path, name, content);
  } catch (e) {
    alert(`Failed to open file: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/* ================= 视图：缩放 ================= */

const zoomLevel = ref(1);

async function applyZoom() {
  try {
    await getCurrentWebviewWindow().setZoom(zoomLevel.value);
  } catch {
    // 非 Tauri 环境（浏览器调试）回退为 CSS zoom
    document.documentElement.style.zoom = String(zoomLevel.value);
  }
}

function zoomIn() {
  zoomLevel.value = Math.min(3, Math.round((zoomLevel.value + 0.1) * 10) / 10);
  void applyZoom();
}

function zoomOut() {
  zoomLevel.value = Math.max(0.5, Math.round((zoomLevel.value - 0.1) * 10) / 10);
  void applyZoom();
}

function resetZoom() {
  zoomLevel.value = 1;
  void applyZoom();
}

/* ================= 全局快捷键 ================= */

/** 派发编辑器命令（查找 / 撤销 / 重做）给 EditorPane */
function dispatchEditorCommand(cmd: 'find' | 'undo' | 'redo') {
  window.dispatchEvent(new CustomEvent(`shortcut:${cmd}`));
}

/** 是否为原生可编辑控件（不含 CodeMirror，其由自身 keymap 处理） */
function isNativeEditable(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.closest('.cm-editor')) return false;
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable;
}

/**
 * 全局快捷键处理 — 按当前设置中的快捷键方案，分发
 * 通用 / 文件 / 视图 / 编辑 四组动作。
 * 编辑器聚焦时，run / save / find / undo / redo 由 CodeMirror keymap 处理。
 */
function onGlobalKeydown(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return;
  const bindings = getKeymapScheme(settings.keymapScheme).bindings;
  const matches = (a: ShortcutAction) => eventMatchesBinding(e, bindings[a]);

  const inEditor = e.target instanceof HTMLElement && !!e.target.closest('.cm-editor');
  const inNativeEditable = isNativeEditable(e.target);

  // 通用
  if (matches('saveAs')) { e.preventDefault(); void saveAs(); return; }
  if (!inEditor && matches('save')) { e.preventDefault(); void saveFile(); return; }
  if (!inEditor && matches('runRequest')) { e.preventDefault(); void run(); return; }
  if (!inEditor && !inNativeEditable && matches('find')) {
    e.preventDefault();
    dispatchEditorCommand('find');
    return;
  }
  // 文件
  if (matches('newFile')) { e.preventDefault(); newFile(); return; }
  if (matches('openFile')) { e.preventDefault(); void openFilePicker(); return; }
  // 视图
  if (matches('toggleSidebar')) { e.preventDefault(); showSidebar.value = !showSidebar.value; return; }
  if (matches('zoomIn')) { e.preventDefault(); zoomIn(); return; }
  if (matches('zoomOut')) { e.preventDefault(); zoomOut(); return; }
  if (matches('resetZoom')) { e.preventDefault(); resetZoom(); return; }
  // 编辑（原生输入控件使用浏览器自带撤销/重做）
  if (!inEditor && !inNativeEditable && matches('undo')) {
    e.preventDefault();
    dispatchEditorCommand('undo');
    return;
  }
  if (!inEditor && !inNativeEditable && matches('redo')) {
    e.preventDefault();
    dispatchEditorCommand('redo');
    return;
  }
}
</script>

<template>
  <div class="app-layout">
    <!-- Custom titlebar -->
    <TitleBar
      :sidebar-visible="showSidebar"
      @toggle-sidebar="showSidebar = !showSidebar"
      @open-settings="workspaceStore.openSettings()"
    />

    <!-- Main body: horizontal split — sidebar | center -->
    <div class="app-body">
      <Splitpanes class="main-split" :first-splitter="true">
        <!-- Sidebar -->
        <Pane v-if="showSidebar" :size="20" :min="15" :max="35">
          <WorkspaceSidebar />
        </Pane>

        <!-- Center: vertical split — editor | history+response area -->
        <Pane :size="80">
          <Splitpanes class="center-split" horizontal :first-splitter="true">
            <!-- Top: editor -->
            <Pane :size="55" :min="20">
              <div class="pane-container">
                <EditorTabBar />
                <div class="pane-body">
                  <SettingsPane v-if="workspaceStore.isSettingsActive" />
                  <EditorPane v-else />
                </div>
              </div>
            </Pane>

            <!-- Bottom: history | response detail -->
            <Pane :size="45" :min="15">
              <Splitpanes class="response-split">
                <Pane :size="30" :min="15">
                  <div class="pane-container">
                    <div class="pane-body">
                      <HistoryPanel />
                    </div>
                  </div>
                </Pane>
                <Pane :size="70" :min="25">
                  <div class="pane-container">
                    <div class="pane-body">
                      <ResponsePane />
                    </div>
                  </div>
                </Pane>
              </Splitpanes>
            </Pane>
          </Splitpanes>
        </Pane>
      </Splitpanes>
    </div>
  </div>
</template>

<style>
/* Import splitpanes CSS is done in script */

.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}
</style>

<style scoped>
.app-body {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.main-split {
  height: 100%;
}

.pane-container {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.pane-body {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}
</style>
