/**
 * useRunCurrent — 执行请求的 composable。
 *
 * 流程:
 * 1. 冲刷编辑器尚未写回 store 的最新内容（防抖窗口内刚敲的字符）
 * 2. 从 requestStore 获取当前 block（或指定索引 / 请求集合中的某一项）
 * 3. 并行守卫 — 同一请求仍在执行中且未开启「允许并行请求」时先询问用户
 * 4. 发起即登记 — requestsStore.startRun() 立刻写入 running 记录（面板马上有反馈）
 * 5. 调用 BackendAdapter.execute()，完成后回填结果 + 更新 gutter 状态徽标
 */

import { useRequestStore } from '../stores/request';
import { useRequestsStore, type RequestEntry } from '../stores/requests';
import { useWorkspaceStore } from '../stores/workspace';
import { detectAdapter } from '../lib/backend';
import { setRunStatus } from '../lib/codemirror/run-status';
import { dirName } from '../lib/path';
import type { Block } from '../lib/parse';
import type { BackendAdapter } from '../types/http';

const METHOD_LINE_RE = /^\s*(GET|HEAD|POST|PUT|DELETE|CONNECT|PATCH|OPTIONS|TRACE)\s+/i;

/** 定位 block 内请求行的行号（1-based），供执行状态徽标使用 */
function requestLineOf(source: string, block: Block): number | null {
  const lines = source.split('\n');
  for (let i = block.startLine; i < block.endLine && i < lines.length; i++) {
    if (METHOD_LINE_RE.test(lines[i])) return i + 1;
  }
  return null;
}

/** 并行询问 — 由 UI（RequestPanel）注册处理器弹窗询问用户 */
export interface ParallelConfirm {
  message: string;
  resolve: (allow: boolean) => void;
}

let _adapter: BackendAdapter | null = null;
let _parallelConfirm: ((p: ParallelConfirm) => void) | null = null;
let _sourceFlusher: (() => void) | null = null;

export function setBackendAdapter(adapter: BackendAdapter) {
  _adapter = adapter;
}

/** 注册并行询问处理器；传 null 注销 */
export function setParallelConfirmHandler(handler: ((p: ParallelConfirm) => void) | null) {
  _parallelConfirm = handler;
}

/**
 * 注册「立即把编辑器最新内容同步到 request store」的回调（由 EditorPane 安装）；传 null 注销。
 *
 * 编辑器写回 store 是防抖的（见 EditorPane 的 updateListener），若执行前不先冲刷，
 * 就会拿旧内容 / 旧行号定位请求块 —— 表现为刚编辑完点 ▶ 没反应，要点第二次才发出请求。
 */
export function setSourceFlusher(flush: (() => void) | null) {
  _sourceFlusher = flush;
}

/** 惰性初始化 adapter — 如果未设置则自动检测 */
function getAdapter(): BackendAdapter {
  if (!_adapter) {
    _adapter = detectAdapter();
  }
  return _adapter;
}

export function useRunCurrent() {
  const requestStore = useRequestStore();
  const requestsStore = useRequestsStore();
  const workspaceStore = useWorkspaceStore();

  /** 二进制响应落盘目录：当前 `.http` 文件同级的 `.http-history`。 */
  function saveDir(): string | undefined {
    const dir = workspaceStore.currentFilePath ? dirName(workspaceStore.currentFilePath) : '';
    return dir ? `${dir}/.http-history` : undefined;
  }

  /** 请求身份键 — 文件路径 + 块名（无名时用块索引），用于归并同一请求的多次执行 */
  function identityKeyOf(blockIndex: number, block: Block): string {
    const file = workspaceStore.currentFilePath ?? '__untitled__';
    return `${file}::${block.name ?? `#${blockIndex}`}`;
  }

  /** 身份键对应的块在当前编辑器中的索引（文件已切换/块已删除时为 null） */
  function blockIndexByIdentity(identityKey: string): number | null {
    const file = workspaceStore.currentFilePath ?? '__untitled__';
    for (let i = 0; i < requestStore.blocks.length; i++) {
      const b = requestStore.blocks[i];
      if (`${file}::${b.name ?? `#${i}`}` === identityKey) return i;
    }
    return null;
  }

  /** 并行守卫 — 返回 true 表示可以继续发起 */
  async function confirmParallel(entry: RequestEntry): Promise<boolean> {
    if (entry.allowParallel) return true;
    if (!_parallelConfirm) return true;
    const label = entry.name ?? entry.target ?? '该请求';
    return new Promise<boolean>((resolve) => {
      _parallelConfirm?.({
        message: `「${label}」正在执行中，是否并行发起新的请求？`,
        resolve,
      });
    });
  }

  /** 实际发起 HTTP 请求并回填结果 */
  async function dispatch(
    adapter: BackendAdapter,
    source: string,
    requestId: string,
    runId: string,
    runLine: number | null,
  ) {
    if (runLine != null) setRunStatus(runLine, 'running');
    try {
      const res = await adapter.execute(source, { saveDir: saveDir() });
      requestsStore.finishOk(requestId, runId, res);
      // 并行执行时，仍有其它运行未结束则继续显示转圈
      if (runLine != null) {
        setRunStatus(runLine, requestsStore.isRunning(requestId) ? 'running' : 'done');
      }
    } catch (e) {
      requestsStore.finishError(requestId, runId, e instanceof Error ? e.message : String(e));
      if (runLine != null) {
        setRunStatus(runLine, requestsStore.isRunning(requestId) ? 'running' : 'error');
      }
    }
  }

  /** 执行编辑器中的某个请求块（默认当前光标/选中的块） */
  async function run(blockIndex?: number) {
    // 编辑器写回 store 有防抖 — 执行前先冲刷，保证块索引与源码都是最新的
    _sourceFlusher?.();

    const idx = blockIndex ?? requestStore.currentBlockIndex;
    const block = requestStore.blocks[idx];

    if (!block) {
      requestsStore.setNotice('未找到请求 — 光标不在任何请求块内');
      return;
    }

    let adapter: BackendAdapter;
    try {
      adapter = getAdapter();
    } catch (e) {
      requestsStore.setNotice(e instanceof Error ? e.message : 'Backend adapter not initialized');
      return;
    }

    const identityKey = identityKeyOf(idx, block);
    const existing = requestsStore.findByIdentity(identityKey);
    if (existing && requestsStore.isRunning(existing.requestId)) {
      const allowed = await confirmParallel(existing);
      if (!allowed) return;
    }

    // 序列化当前 block 的源码
    const slice = requestStore.getBlockSource(idx) ?? '';
    const runLine = requestLineOf(requestStore.source, block);

    // 发起即登记 — 面板立刻显示「执行中」
    const { requestId, runId } = requestsStore.startRun({
      identityKey,
      name: block.name ?? null,
      method: block.method,
      target: block.target,
      filePath: workspaceStore.currentFilePath,
      source: slice,
    });

    await dispatch(adapter, slice, requestId, runId, runLine);
  }

  /**
   * 二次执行请求集合中的某个请求。
   * 块仍在当前编辑器中 → 用最新源码执行；否则回退到最近一次运行的源码快照。
   */
  async function rerun(requestId: string) {
    const entry = requestsStore.getEntry(requestId);
    if (!entry) return;

    // 先冲刷编辑器内容 — 身份键要按最新的块列表解析
    _sourceFlusher?.();

    const idx = blockIndexByIdentity(entry.identityKey);
    if (idx != null) {
      await run(idx);
      return;
    }

    const last = entry.runs[0];
    if (!last) return;

    let adapter: BackendAdapter;
    try {
      adapter = getAdapter();
    } catch (e) {
      requestsStore.setNotice(e instanceof Error ? e.message : 'Backend adapter not initialized');
      return;
    }

    if (requestsStore.isRunning(entry.requestId)) {
      const allowed = await confirmParallel(entry);
      if (!allowed) return;
    }

    const { requestId: rid, runId } = requestsStore.startRun({
      identityKey: entry.identityKey,
      name: entry.name,
      method: last.method,
      target: last.target,
      filePath: entry.filePath,
      source: last.source,
    });

    await dispatch(adapter, last.source, rid, runId, null);
  }

  return { run, rerun };
}
