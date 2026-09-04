/**
 * 请求执行状态注册表 — 以「请求行行号（1-based）」为键，
 * 驱动 gutter 中 ▶ 按钮右下角的状态徽标（转圈 / 绿√ / 红✕）。
 *
 * CodeMirror gutter markers 只在文档/视口变化时重建，
 * 因此状态变化通过订阅回调触发 runGutter compartment 重配置。
 */

export type RunStatus = 'running' | 'done' | 'error';

const statuses = new Map<number, RunStatus>();
const listeners = new Set<() => void>();

function notify() {
  for (const fn of listeners) fn();
}

export function setRunStatus(line: number, status: RunStatus) {
  if (statuses.get(line) === status) return;
  statuses.set(line, status);
  notify();
}

export function getRunStatus(line: number): RunStatus | undefined {
  return statuses.get(line);
}

/** 清空所有状态（文档变更后行号失效时调用） */
export function clearRunStatuses() {
  if (statuses.size === 0) return;
  statuses.clear();
  notify();
}

/** 订阅状态变化，返回取消订阅函数 */
export function onRunStatusChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
