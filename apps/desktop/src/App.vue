<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { Splitpanes, Pane } from 'splitpanes';
import 'splitpanes/dist/splitpanes.css';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
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
  // 优雅退出：关闭时保存现场（打开的标签页与激活状态）
  window.addEventListener('beforeunload', onBeforeUnload);
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

  // 优先恢复上次退出现场；恢复失败（首次启动 / 数据损坏）走默认文件初始化
  const restored = workspaceStore.restoreSession();

  const fs = getFs();
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
});

/** 退出前保存现场 */
function onBeforeUnload() {
  workspaceStore.saveSession();
}

/* ================= 文件操作 ================= */

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
    await fs.writeFile(path, requestStore.source);
    workspaceStore.markClean();
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
  try {
    await fs.writeFile(name, requestStore.source);
    const parts = name.split('/');
    workspaceStore.openFile(name, parts[parts.length - 1] || name, requestStore.source);
    // 「另存为」成功后关闭来源的未命名标签页
    if (prevPath && UNTITLED_RE.test(prevPath)) {
      workspaceStore.closeTab(prevPath);
    }
    workspaceStore.markClean();
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
