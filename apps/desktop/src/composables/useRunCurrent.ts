/**
 * useRunCurrent — 执行当前光标所在请求的 composable。
 *
 * 流程:
 * 1. 从 requestStore 获取当前 block
 * 2. 序列化该 block 的源码（http-web 只执行第一个请求）
 * 3. 调用 BackendAdapter.execute()
 * 4. 更新 responseStore + historyStore（保存完整响应快照）
 */

import { useRequestStore } from '../stores/request';
import { useResponseStore } from '../stores/response';
import { useHistoryStore } from '../stores/history';
import { detectAdapter } from '../lib/backend';
import { setRunStatus } from '../lib/codemirror/run-status';
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

let _adapter: BackendAdapter | null = null;

export function setBackendAdapter(adapter: BackendAdapter) {
  _adapter = adapter;
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
  const responseStore = useResponseStore();
  const historyStore = useHistoryStore();

  async function run(blockIndex?: number) {
    const block = blockIndex != null
      ? requestStore.blocks[blockIndex]
      : requestStore.currentBlock;

    if (!block) {
      responseStore.fail('No request found — cursor is not inside a request block');
      return;
    }

    let adapter: BackendAdapter;
    try {
      adapter = getAdapter();
    } catch (e) {
      responseStore.fail(e instanceof Error ? e.message : 'Backend adapter not initialized');
      return;
    }

    // 序列化当前 block 的源码
    const slice = requestStore.getBlockSource(blockIndex) ?? '';

    // 执行状态徽标：请求行显示转圈 → 完成绿√ / 失败红✕
    const runLine = requestLineOf(requestStore.source, block);

    if (runLine != null) setRunStatus(runLine, 'running');
    responseStore.start();
    try {
      const res = await adapter.execute(slice);
      responseStore.ok(res);
      if (runLine != null) setRunStatus(runLine, 'done');
      historyStore.add({
        method: block.method,
        target: block.target,
        status: res.status,
        elapsedMs: res.elapsed_ms,
        source: slice,
        response: res,
      });
    } catch (e) {
      responseStore.fail(e instanceof Error ? e.message : String(e));
      if (runLine != null) setRunStatus(runLine, 'error');
    }
  }

  /** 从历史记录重放（重新发送请求） */
  async function replay(source: string, method: string, target: string) {
    let adapter: BackendAdapter;
    try {
      adapter = getAdapter();
    } catch (e) {
      responseStore.fail(e instanceof Error ? e.message : 'Backend adapter not initialized');
      return;
    }

    responseStore.start();
    try {
      const res = await adapter.execute(source);
      responseStore.ok(res);
      historyStore.add({
        method,
        target,
        status: res.status,
        elapsedMs: res.elapsed_ms,
        source,
        response: res,
      });
    } catch (e) {
      responseStore.fail(e instanceof Error ? e.message : String(e));
    }
  }

  return { run, replay };
}
