<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { EditorView, keymap, lineNumbers } from '@codemirror/view';
import { Annotation, Compartment, EditorState } from '@codemirror/state';
import { defaultKeymap, history, historyKeymap, redo, undo } from '@codemirror/commands';
import { openSearchPanel, searchKeymap } from '@codemirror/search';
import { useRequestStore } from '../stores/request';
import { useEnvironmentStore } from '../stores/environment';
import { useWorkspaceStore } from '../stores/workspace';
import { useSettingsStore } from '../stores/settings';
import { useHistoryStore } from '../stores/history';
import { useRunCurrent } from '../composables/useRunCurrent';
import { httpExtensions, runGutter } from '../lib/codemirror/extensions';
import { targetsOf } from '../lib/codemirror/completions';
import { buildEditorTheme } from '../lib/codemirror/editor-theme';
import { clearRunStatuses, onRunStatusChange } from '../lib/codemirror/run-status';
import { blockToCurl } from '../lib/codemirror/http-to-curl';
import { getKeymapScheme } from '../lib/keymaps';
import { getFs } from '../lib/backend/fs';

const requestStore = useRequestStore();
const envStore = useEnvironmentStore();
const workspaceStore = useWorkspaceStore();
const settings = useSettingsStore();
const historyStore = useHistoryStore();
const { run } = useRunCurrent();

const editorEl = ref<HTMLElement>();
let view: EditorView | null = null;

// 运行时可重配置：编辑器主题 + 应用快捷键 + 撤销栈 + 执行状态 gutter
const themeCompartment = new Compartment();
const keymapCompartment = new Compartment();
const historyCompartment = new Compartment();
const runGutterCompartment = new Compartment();

/** 外部同步事务标记（标签页切换 / history replay / 表单编辑）—
 *  不进入撤销栈，也不触发 dirty 标记 */
const externalSync = Annotation.define<boolean>();

// 防抖更新 store
let updateTimer: ReturnType<typeof setTimeout> | null = null;

// 执行状态订阅取消函数
let unsubscribeRunStatus: (() => void) | null = null;

// 行内 Run 按钮点击事件
function onRunBlock(e: Event) {
  const detail = (e as CustomEvent).detail;
  if (detail && typeof detail.line === 'number') {
    // 找到该行对应的 block
    const lineIdx = detail.line - 1; // CodeMirror 行号 1-based → 0-based
    for (let i = 0; i < requestStore.blocks.length; i++) {
      const b = requestStore.blocks[i];
      if (lineIdx >= b.startLine && lineIdx < b.endLine) {
        requestStore.selectBlock(i);
        run(i);
        break;
      }
    }
  }
}

// 保存当前文件
async function saveCurrentFile() {
  const fs = getFs();
  const path = workspaceStore.currentFilePath;
  if (!fs || !path) return;
  // 取标签页内容：编辑器回写 requestStore 有 100ms 防抖，直接用 source 可能写入旧内容
  const content = workspaceStore.getTab(path)?.content ?? requestStore.source;
  try {
    await fs.writeFile(path, content);
    workspaceStore.markClean();
    workspaceStore.notifyFsChange();
  } catch (e) {
    alert(`Failed to save: ${e instanceof Error ? e.message : String(e)}`);
  }
}

// ---- 右键上下文菜单（复制为 cURL） ----
interface CtxMenuState {
  x: number;
  y: number;
  blockIndex: number;
}
const ctxMenu = ref<CtxMenuState | null>(null);
const ctxMenuEl = ref<HTMLElement>();

function onContextMenu(e: MouseEvent) {
  if (!view) return;
  const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
  if (pos == null) return;
  const lineIdx = view.state.doc.lineAt(pos).number - 1;
  const idx = requestStore.blocks.findIndex((b) => lineIdx >= b.startLine && lineIdx < b.endLine);
  if (idx < 0) return;
  e.preventDefault();
  ctxMenu.value = { x: e.clientX, y: e.clientY, blockIndex: idx };
}

async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // 回退方案：隐藏 textarea + execCommand（剪贴板 API 受限的环境）
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch {
      ok = false;
    } finally {
      document.body.removeChild(ta);
    }
    return ok;
  }
}

async function copyAsCurl() {
  const menu = ctxMenu.value;
  ctxMenu.value = null;
  if (!menu) return;
  const block = requestStore.blocks[menu.blockIndex];
  if (!block) return;
  const curl = blockToCurl(block, { resolveVar: (name) => envStore.resolve(name) });
  await copyToClipboard(curl);
}

function onDocMouseDown(e: MouseEvent) {
  if (!ctxMenu.value) return;
  if (ctxMenuEl.value && e.target instanceof Node && ctxMenuEl.value.contains(e.target)) return;
  ctxMenu.value = null;
}

function onCtxMenuEscKey(e: KeyboardEvent) {
  if (e.key === 'Escape') ctxMenu.value = null;
}

watch(ctxMenu, (open) => {
  if (open) {
    document.addEventListener('mousedown', onDocMouseDown, true);
    document.addEventListener('keydown', onCtxMenuEscKey);
  } else {
    document.removeEventListener('mousedown', onDocMouseDown, true);
    document.removeEventListener('keydown', onCtxMenuEscKey);
  }
});

// 按当前快捷键方案构建应用级编辑器绑定（运行/保存）
function buildAppKeymap() {
  const bindings = getKeymapScheme(settings.keymapScheme).bindings;
  return keymap.of([
    {
      key: bindings.runRequest,
      run: () => { run(); return true; },
    },
    {
      key: bindings.save,
      preventDefault: true,
      run: () => { saveCurrentFile(); return true; },
    },
  ]);
}

// 外部派发的编辑器命令（全局快捷键 / 菜单栏触发）
function onShortcutEvent(e: Event) {
  if (!view) return;
  switch (e.type) {
    case 'shortcut:find':
      view.focus();
      openSearchPanel(view);
      break;
    case 'shortcut:undo':
      undo(view);
      break;
    case 'shortcut:redo':
      redo(view);
      break;
  }
}

// 补全语料：其它标签页与执行历史里学习到的 URL（当前文档由扩展自行收集）
function knownUrls(): string[] {
  const others = workspaceStore.tabs.filter(
    (t) => t.type === 'file' && t.path !== workspaceStore.activeTabPath,
  );
  return [
    ...others.flatMap((t) => targetsOf(t.content)),
    ...historyStore.items.map((i) => i.target),
  ];
}

onMounted(() => {
  if (!editorEl.value) return;

  const extensions = [
    lineNumbers(),
    ...httpExtensions((name) => envStore.isDefined(name), {
      knownUrls,
      variableNames: () => Object.keys(envStore.vars),
    }),
    runGutterCompartment.of(runGutter()),
    historyCompartment.of(history()),
    keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
    keymapCompartment.of(buildAppKeymap()),
    EditorView.updateListener.of((update) => {
      // 外部同步事务标记：仅同步文档，不触发 dirty / store 回写
      const isSync = update.transactions.some((tr) => tr.annotation(externalSync));
      if (update.docChanged) {
        // 文档变更后行号失效 → 清空执行状态徽标
        clearRunStatuses();
      }
      if (update.docChanged && !isSync) {
        const newSource = update.state.sliceDoc();
        workspaceStore.updateActiveContent(newSource);
        workspaceStore.markDirty();
        if (updateTimer) clearTimeout(updateTimer);
        updateTimer = setTimeout(() => { requestStore.setSource(newSource); }, 100);
      }
      if (update.selectionSet) {
        const pos = update.state.selection.main.head;
        const line = update.state.doc.lineAt(pos);
        requestStore.setCursorLine(line.number - 1);
        // 光标移动时清除手动选择，让 currentBlock 跟随光标
        requestStore.selectBlock(null);
      }
    }),
    themeCompartment.of(buildEditorTheme(settings.resolvedEditorScheme)),
  ];

  view = new EditorView({
    state: EditorState.create({
      doc: requestStore.source,
      extensions,
    }),
    parent: editorEl.value,
  });

  // 监听行内 Run 按钮事件
  editorEl.value.addEventListener('cm-run-block', onRunBlock);

  // 右键菜单（复制为 cURL）
  editorEl.value.addEventListener('contextmenu', onContextMenu);

  // 监听外部编辑器命令（查找 / 撤销 / 重做）
  window.addEventListener('shortcut:find', onShortcutEvent);
  window.addEventListener('shortcut:undo', onShortcutEvent);
  window.addEventListener('shortcut:redo', onShortcutEvent);

  // 执行状态变化 → 重配置 run gutter 以刷新徽标
  // （microtask 延迟避免在 update 周期内 dispatch）
  unsubscribeRunStatus = onRunStatusChange(() => {
    queueMicrotask(() => {
      view?.dispatch({ effects: runGutterCompartment.reconfigure(runGutter()) });
    });
  });
});

onUnmounted(() => {
  editorEl.value?.removeEventListener('cm-run-block', onRunBlock);
  editorEl.value?.removeEventListener('contextmenu', onContextMenu);
  window.removeEventListener('shortcut:find', onShortcutEvent);
  window.removeEventListener('shortcut:undo', onShortcutEvent);
  window.removeEventListener('shortcut:redo', onShortcutEvent);
  document.removeEventListener('mousedown', onDocMouseDown, true);
  document.removeEventListener('keydown', onCtxMenuEscKey);
  unsubscribeRunStatus?.();
  view?.destroy();
  if (updateTimer) clearTimeout(updateTimer);
});

// 外部 source 变更时同步到编辑器（如标签页切换 / history replay / 表单编辑）
watch(
  () => requestStore.source,
  (newSource) => {
    if (!view) return;
    // 清除待执行的防抖更新，避免标签切换后旧内容覆盖
    if (updateTimer) { clearTimeout(updateTimer); updateTimer = null; }
    const current = view.state.sliceDoc();
    if (newSource !== current) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: newSource },
        annotations: externalSync.of(true),
      });
      // 重置撤销栈：避免跨标签页撤销到其它文件的内容
      view.dispatch({ effects: historyCompartment.reconfigure(history()) });
    }
  },
);

// 编辑器 color scheme 变更 → 重配置主题
watch(
  () => settings.resolvedEditorScheme,
  (scheme) => {
    view?.dispatch({ effects: themeCompartment.reconfigure(buildEditorTheme(scheme)) });
  },
);

// 编辑器字体变更（--font-editor 由 settings store 写入）→ 重新测量文本几何
watch(
  () => settings.editorFontFamily,
  () => {
    view?.requestMeasure();
  },
);

// 快捷键方案变更 → 重配置应用级绑定
watch(
  () => settings.keymapScheme,
  () => {
    view?.dispatch({ effects: keymapCompartment.reconfigure(buildAppKeymap()) });
  },
);
</script>

<template>
  <div class="editor-pane">
    <div ref="editorEl" class="editor-host" />

    <!-- 右键上下文菜单 -->
    <div
      v-if="ctxMenu"
      ref="ctxMenuEl"
      class="context-menu"
      :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }"
      @click.stop
    >
      <button class="menu-item" @click="copyAsCurl">Copy as cURL</button>
    </div>
  </div>
</template>

<style scoped>
.editor-pane {
  position: relative;
  height: 100%;
  overflow: hidden;
  border: none;
  border-radius: 6px;
  background: var(--bg-base);
}

.editor-host {
  height: 100%;
}

.editor-pane :deep(.cm-editor) {
  height: 100%;
}

.editor-pane :deep(.cm-focused) {
  outline: none;
}

.context-menu {
  position: fixed;
  background: var(--bg-chrome);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 4px 12px var(--shadow);
  padding: 4px 0;
  z-index: 10000;
  min-width: 140px;
}

.menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 4px 12px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--fg);
}

.menu-item:hover {
  background: var(--bg-hover);
}
</style>
