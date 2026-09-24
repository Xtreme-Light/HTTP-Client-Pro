/**
 * requests store — 「请求集合」数据层（取代旧的 history store）。
 *
 * 每个请求（同一文件里的同一个 block）在首次发起时由数据层分配一个
 * 唯一 requestId（前台不显示）；同一请求的反复执行都归并到它自己
 * 名下，runs[0] 永远是最近一次执行。发起请求即登记 running 记录，
 * 面板立刻有反馈（转圈），完成后回填结果。
 */
import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import type { DispatchResponse } from '../types/http';

export type RunStatus = 'running' | 'done' | 'error';

/** 单次执行记录 */
export interface RunRecord {
  runId: string;
  /** 发起时间（毫秒时间戳） */
  ts: number;
  status: RunStatus;
  /** 发起时的请求方法/目标快照（历史运行不随后续编辑变化） */
  method: string;
  target: string;
  /** 发起时的请求源码快照 — 供 Request 页签与「执行」按钮复用 */
  source: string;
  httpStatus: number | null;
  elapsedMs: number;
  error: string | null;
  response: DispatchResponse | null;
}

/** 请求集合中的一个请求 */
export interface RequestEntry {
  /** 数据层分配的唯一 ID（前台不显示） */
  requestId: string;
  /** 身份键 — 判定「同一个请求」：文件路径 + 块名（无名时用块索引） */
  identityKey: string;
  /** 显示名称 — 默认取 `###` 名，可在「修改运行配置」中自定义 */
  name: string | null;
  /** 用户是否自定义过名称（自定义后不再跟随 `###` 名更新） */
  nameOverridden: boolean;
  method: string;
  target: string;
  filePath: string | null;
  /** 允许并行请求 — 为 false 时重复点击先询问用户 */
  allowParallel: boolean;
  /** 执行记录，最近一次在前 */
  runs: RunRecord[];
}

/** startRun 的入参 — 描述本次要发起的请求 */
export interface StartRunMeta {
  identityKey: string;
  name: string | null;
  method: string;
  target: string;
  filePath: string | null;
  source: string;
}

const STORAGE_KEY = 'http-client-pro:requests';
const MAX_ENTRIES = 50;
const MAX_RUNS_PER_ENTRY = 10;

/** 生成唯一 ID — 前台不显示，仅用于归并同一请求的多次执行 */
export function genId(): string {
  try {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
      return crypto.randomUUID();
    }
  } catch {
    /* insecure context etc. */
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/** 序列化 entries — 可独立测试 */
export function serialize(entries: RequestEntry[]): string {
  return JSON.stringify(entries);
}

function isRecord(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null;
}

function parseRun(raw: unknown, fallback: { method: string; target: string }): RunRecord | null {
  if (!isRecord(raw) || typeof raw.runId !== 'string') return null;
  const status: RunStatus =
    raw.status === 'done' || raw.status === 'running' ? raw.status : 'error';
  return {
    runId: raw.runId,
    ts: typeof raw.ts === 'number' ? raw.ts : 0,
    status,
    method: typeof raw.method === 'string' ? raw.method : fallback.method,
    target: typeof raw.target === 'string' ? raw.target : fallback.target,
    source: typeof raw.source === 'string' ? raw.source : '',
    httpStatus: typeof raw.httpStatus === 'number' ? raw.httpStatus : null,
    elapsedMs: typeof raw.elapsedMs === 'number' ? raw.elapsedMs : 0,
    error: typeof raw.error === 'string' ? raw.error : null,
    response: isRecord(raw.response) ? (raw.response as unknown as DispatchResponse) : null,
  };
}

/** 反序列化 — 格式错误时返回空数组；非法字段回退默认值 */
export function deserialize(raw: string | null): RequestEntry[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const entries: RequestEntry[] = [];
    for (const item of parsed) {
      if (!isRecord(item) || typeof item.requestId !== 'string' || !Array.isArray(item.runs)) {
        continue;
      }
      const method = typeof item.method === 'string' ? item.method : 'GET';
      const target = typeof item.target === 'string' ? item.target : '';
      const runs = item.runs
        .map((r) => parseRun(r, { method, target }))
        .filter((r): r is RunRecord => r !== null);
      entries.push({
        requestId: item.requestId,
        identityKey: typeof item.identityKey === 'string' ? item.identityKey : item.requestId,
        name: typeof item.name === 'string' ? item.name : null,
        nameOverridden: item.nameOverridden === true,
        method,
        target,
        filePath: typeof item.filePath === 'string' ? item.filePath : null,
        allowParallel: item.allowParallel === true,
        runs,
      });
    }
    return entries;
  } catch {
    return [];
  }
}

export const useRequestsStore = defineStore('requests', () => {
  const entries = ref<RequestEntry[]>([]);
  const selectedRequestId = ref<string | null>(null);
  /** 钉住查看的历史运行 runId — null 表示跟随最近一次 */
  const selectedRunId = ref<string | null>(null);
  /** 执行未能发起时的提示（找不到请求块 / 后端未初始化） */
  const notice = ref<string | null>(null);

  const selectedEntry = computed<RequestEntry | null>(
    () => entries.value.find((e) => e.requestId === selectedRequestId.value) ?? null,
  );

  /** 右侧详情要展示的运行 — 钉住的历史运行，或最近一次 */
  const selectedRun = computed<RunRecord | null>(() => {
    const entry = selectedEntry.value;
    if (!entry) return null;
    if (selectedRunId.value) {
      const pinned = entry.runs.find((r) => r.runId === selectedRunId.value);
      if (pinned) return pinned;
    }
    return entry.runs[0] ?? null;
  });

  function getEntry(requestId: string): RequestEntry | null {
    return entries.value.find((e) => e.requestId === requestId) ?? null;
  }

  function findByIdentity(identityKey: string): RequestEntry | null {
    return entries.value.find((e) => e.identityKey === identityKey) ?? null;
  }

  /** 该请求是否有执行仍在进行中（含并行执行） */
  function isRunning(requestId: string): boolean {
    const entry = getEntry(requestId);
    return !!entry && entry.runs.some((r) => r.status === 'running');
  }

  /**
   * 登记一次发起 — 已存在的请求归并（刷新 method/target，名称未自定义时跟随 `###`），
   * 新请求分配唯一 requestId 并插入集合；随后选中该请求并跟随最近一次运行。
   */
  function startRun(meta: StartRunMeta): { requestId: string; runId: string } {
    notice.value = null;
    let entry = findByIdentity(meta.identityKey);
    if (!entry) {
      entry = {
        requestId: genId(),
        identityKey: meta.identityKey,
        name: meta.name,
        nameOverridden: false,
        method: meta.method,
        target: meta.target,
        filePath: meta.filePath,
        allowParallel: false,
        runs: [],
      };
      entries.value.unshift(entry);
      if (entries.value.length > MAX_ENTRIES) {
        entries.value = entries.value.slice(0, MAX_ENTRIES);
      }
    } else {
      entry.method = meta.method;
      entry.target = meta.target;
      if (!entry.nameOverridden) entry.name = meta.name;
      if (meta.filePath) entry.filePath = meta.filePath;
    }

    const runId = genId();
    entry.runs.unshift({
      runId,
      ts: Date.now(),
      status: 'running',
      method: meta.method,
      target: meta.target,
      source: meta.source,
      httpStatus: null,
      elapsedMs: 0,
      error: null,
      response: null,
    });
    if (entry.runs.length > MAX_RUNS_PER_ENTRY) {
      entry.runs = entry.runs.slice(0, MAX_RUNS_PER_ENTRY);
    }

    select(entry.requestId);
    persist();
    return { requestId: entry.requestId, runId };
  }

  /** 回填成功结果 */
  function finishOk(requestId: string, runId: string, response: DispatchResponse) {
    const run = getEntry(requestId)?.runs.find((r) => r.runId === runId);
    if (!run || run.status !== 'running') return;
    run.status = 'done';
    run.response = response;
    run.httpStatus = response.status;
    run.elapsedMs = response.elapsed_ms;
    persist();
  }

  /** 回填失败结果 */
  function finishError(requestId: string, runId: string, message: string) {
    const run = getEntry(requestId)?.runs.find((r) => r.runId === runId);
    if (!run || run.status !== 'running') return;
    run.status = 'error';
    run.error = message;
    persist();
  }

  /** 选中请求 — 详情跟随最近一次运行 */
  function select(requestId: string | null) {
    selectedRequestId.value = requestId;
    selectedRunId.value = null;
  }

  /** 查看（钉住）某次历史运行；runId 为 null 时回到最近一次 */
  function viewRun(requestId: string, runId: string | null) {
    selectedRequestId.value = requestId;
    selectedRunId.value = runId;
  }

  /** 保存「修改运行配置」 */
  function updateConfig(
    requestId: string,
    patch: { name?: string | null; allowParallel?: boolean },
  ) {
    const entry = getEntry(requestId);
    if (!entry) return;
    if (patch.name !== undefined) {
      const trimmed = patch.name?.trim() ?? '';
      entry.name = trimmed || null;
      entry.nameOverridden = true;
    }
    if (patch.allowParallel !== undefined) {
      entry.allowParallel = patch.allowParallel;
    }
    persist();
  }

  function setNotice(message: string | null) {
    notice.value = message;
  }

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, serialize(entries.value));
    } catch {
      /* quota or unavailable */
    }
  }

  function load() {
    try {
      entries.value = deserialize(localStorage.getItem(STORAGE_KEY));
      // 重启后仍在「执行中」的运行已不存在 → 标记为中断
      for (const entry of entries.value) {
        for (const run of entry.runs) {
          if (run.status === 'running') {
            run.status = 'error';
            run.error = run.error ?? 'Interrupted (app restarted)';
          }
        }
      }
    } catch {
      /* localStorage unavailable */
    }
  }

  function clear() {
    entries.value = [];
    selectedRequestId.value = null;
    selectedRunId.value = null;
    notice.value = null;
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      /* */
    }
  }

  return {
    entries,
    selectedRequestId,
    selectedRunId,
    notice,
    selectedEntry,
    selectedRun,
    getEntry,
    findByIdentity,
    isRunning,
    startRun,
    finishOk,
    finishError,
    select,
    viewRun,
    updateConfig,
    setNotice,
    load,
    persist,
    clear,
  };
});
