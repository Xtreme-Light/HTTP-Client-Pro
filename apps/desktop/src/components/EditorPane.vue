<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { EditorView, keymap, lineNumbers } from '@codemirror/view';
import { Annotation, Compartment, EditorState, Prec } from '@codemirror/state';
import { defaultKeymap, history, historyKeymap, redo, undo } from '@codemirror/commands';
import { openSearchPanel, searchKeymap } from '@codemirror/search';
import { useRequestStore } from '../stores/request';
import { useEnvironmentStore } from '../stores/environment';
import { useWorkspaceStore } from '../stores/workspace';
import { useSettingsStore } from '../stores/settings';
import { useRequestsStore } from '../stores/requests';
import { setSourceFlusher, useRunCurrent } from '../composables/useRunCurrent';
import { httpExtensions, runGutter } from '../lib/codemirror/extensions';
import { targetsOf } from '../lib/codemirror/completions';
import { buildEditorTheme } from '../lib/codemirror/editor-theme';
import { clearRunStatuses, onRunStatusChange } from '../lib/codemirror/run-status';
import { blockToCurl } from '../lib/codemirror/http-to-curl';
import { formatDocumentCommand } from '../lib/codemirror/format';
import { gotoNextLineCommand } from '../lib/codemirror/commands';
import { getKeymapScheme } from '../lib/keymaps';

const requestStore = useRequestStore();
const envStore = useEnvironmentStore();
const workspaceStore = useWorkspaceStore();
const settings = useSettingsStore();
const requestsStore = useRequestsStore();
const { run } = useRunCurrent();

const editorEl = ref<HTMLElement>();
let view: EditorView | null = null;

// 运行时可重配置：编辑器主题 + 应用快捷键 + 撤销栈 + 执行状态 gutter + 只读模式 + 长行折行
const themeCompartment = new Compartment();
const keymapCompartment = new Compartment();
const historyCompartment = new Compartment();
const runGutterCompartment = new Compartment();
const readOnlyCompartment = new Compartment();
const lineWrapCompartment = new Compartment();

/** 构建长行折行扩展（关闭时长行不换行，横向滚动查看） */
function buildLineWrapExt(wrap: boolean) {
  return wrap ? [EditorView.lineWrapping] : [];
}

/** 当前激活标签是否为只读示例 */
function isExampleActive(): boolean {
  return workspaceStore.activeTab?.type === 'example';
}

/** 构建只读扩展（示例标签页禁止编辑与输入） */
function buildReadOnlyExt(ro: boolean) {
  return [EditorState.readOnly.of(ro), EditorView.editable.of(!ro)];
}

/** 外部同步事务标记（标签页切换 / history replay / 表单编辑）—
 *  不进入撤销栈，也不触发 dirty 标记 */
const externalSync = Annotation.define<boolean>();

// 防抖更新 store
let updateTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * 立即把编辑器最新内容写回 request store（取消防抖等待）。
 * 执行请求前调用 — 否则防抖窗口内的编辑还没进 store，
 * 会按旧源码 / 旧行号定位请求块，表现为 ▶ 要点第二次才发出请求。
 */
function flushPendingSource() {
  if (updateTimer) { clearTimeout(updateTimer); updateTimer = null; }
  if (!view) return;
  const src = view.state.sliceDoc();
  if (src !== requestStore.source) requestStore.setSource(src);
}

// 执行状态订阅取消函数
let unsubscribeRunStatus: (() => void) | null = null;

// 行内 Run 按钮点击事件
function onRunBlock(e: Event) {
  const detail = (e as CustomEvent).detail;
  if (detail && typeof detail.line === 'number') {
    // 先冲刷未写回的编辑，才能按最新行号定位到块
    flushPendingSource();
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
  // 只读示例 / 设置标签页不可保存
  if (!workspaceStore.canSave) return;
  try {
    // 未命名标签页由 store 统一保存到默认工作区目录
    await workspaceStore.saveTab();
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

/** 复制该 block 的原始文本（按源文件行范围原样截取，不做重新序列化） */
async function copyRawBlock() {
  const menu = ctxMenu.value;
  ctxMenu.value = null;
  if (!menu) return;
  const block = requestStore.blocks[menu.blockIndex];
  if (!block) return;
  const lines = requestStore.source.split('\n');
  const raw = lines.slice(block.startLine, block.endLine).join('\n');
  await copyToClipboard(raw);
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

// 按当前快捷键方案构建应用级编辑器绑定（运行/保存/格式化/跳到下一行）
// Prec.high：defaultKeymap 在扩展数组里更靠前，优先级更高，
// 不加 Prec.high 时 Mod-Enter 会被内置的 insertBlankLine 抢先处理。
function buildAppKeymap() {
  const bindings = getKeymapScheme(settings.keymapScheme).bindings;
  return Prec.high(keymap.of([
    {
      key: bindings.runRequest,
      run: () => { run(); return true; },
    },
    {
      key: bindings.save,
      preventDefault: true,
      run: () => { saveCurrentFile(); return true; },
    },
    {
      key: bindings.formatDocument,
      preventDefault: true,
      run: (v) => (isExampleActive() ? true : formatDocumentCommand(v)),
    },
    {
      key: bindings.gotoNextLine,
      preventDefault: true,
      run: (v) => (isExampleActive() ? true : gotoNextLineCommand(v)),
    },
  ]));
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

// 补全语料：其它标签页与请求集合里学习到的 URL（当前文档由扩展自行收集）
function knownUrls(): string[] {
  const others = workspaceStore.tabs.filter(
    (t) => t.type === 'file' && t.path !== workspaceStore.activeTabPath,
  );
  return [
    ...others.flatMap((t) => targetsOf(t.content)),
    ...requestsStore.entries.map((e) => e.target),
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
    readOnlyCompartment.of(buildReadOnlyExt(isExampleActive())),
    lineWrapCompartment.of(buildLineWrapExt(settings.editorLineWrap)),
    keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
    keymapCompartment.of(buildAppKeymap()),
    EditorView.updateListener.of((update) => {
      // 外部同步事务标记：仅同步文档，不触发 dirty / store 回写
      const isSync = update.transactions.some((tr) => tr.annotation(externalSync));
      if (update.docChanged) {
        // 文档变更后行号失效 → 清空执行状态徽标
        clearRunStatuses();
      }
      if (update.docChanged && !isSync && !isExampleActive()) {
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

  // 执行请求前先冲刷防抖中的编辑器内容（工具栏 / 快捷键 / ▶ 按钮共用）
  setSourceFlusher(flushPendingSource);

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
  setSourceFlusher(null);
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

// 编辑器字体 / 字号变更（--font-editor、--font-scale 由 settings store 写入）→ 重新测量文本几何
watch(
  () => [settings.editorFontFamily, settings.fontScale],
  () => {
    view?.requestMeasure();
  },
);

// Editor 行折叠（长行折行）变更 → 重配置折行扩展
watch(
  () => settings.editorLineWrap,
  (wrap) => {
    view?.dispatch({ effects: lineWrapCompartment.reconfigure(buildLineWrapExt(wrap)) });
  },
);

// 快捷键方案变更 → 重配置应用级绑定
watch(
  () => settings.keymapScheme,
  () => {
    view?.dispatch({ effects: keymapCompartment.reconfigure(buildAppKeymap()) });
  },
);

// 激活标签类型变更（普通文件 ↔ 只读示例）→ 重配置只读模式
watch(
  () => workspaceStore.activeTab?.type,
  (type) => {
    const ro = type === 'example';
    view?.dispatch({ effects: readOnlyCompartment.reconfigure(buildReadOnlyExt(ro)) });
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
      <button class="menu-item" @click="copyRawBlock">Copy</button>
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
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg);
}

.menu-item:hover {
  background: var(--bg-hover);
}
</style>
