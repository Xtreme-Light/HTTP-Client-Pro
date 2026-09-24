import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import {
  useRequestsStore,
  serialize,
  deserialize,
  type StartRunMeta,
} from './requests';
import type { DispatchResponse } from '../types/http';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
});

function meta(over: Partial<StartRunMeta> = {}): StartRunMeta {
  return {
    identityKey: '/a.http::登录',
    name: '登录',
    method: 'POST',
    target: 'http://x/post',
    filePath: '/a.http',
    source: '### 登录\nPOST http://x/post\n',
    ...over,
  };
}

function res(status = 200, elapsed = 12): DispatchResponse {
  return {
    status,
    headers: { 'content-type': 'application/json' },
    body: '{"ok":true}',
    elapsed_ms: elapsed,
    url: 'http://x/post',
  } as DispatchResponse;
}

describe('requests store — serialize/deserialize', () => {
  it('round trips entries', () => {
    const s = useRequestsStore();
    const { requestId, runId } = s.startRun(meta());
    s.finishOk(requestId, runId, res());

    const restored = deserialize(serialize(s.entries));
    expect(restored).toHaveLength(1);
    expect(restored[0].requestId).toBe(requestId);
    expect(restored[0].name).toBe('登录');
    expect(restored[0].allowParallel).toBe(false);
    expect(restored[0].runs[0].status).toBe('done');
    expect(restored[0].runs[0].httpStatus).toBe(200);
    expect(restored[0].runs[0].source).toContain('### 登录');
  });

  it('returns [] for null / malformed input', () => {
    expect(deserialize(null)).toEqual([]);
    expect(deserialize('not json')).toEqual([]);
    expect(deserialize('{"a":1}')).toEqual([]);
    expect(deserialize('[{"nope":true}]')).toEqual([]);
  });

  it('falls back to defaults for bad fields', () => {
    const out = deserialize('[{"requestId":"r1","runs":[{"runId":"run1","status":"weird"},{"x":1}]}]');
    expect(out).toHaveLength(1);
    expect(out[0].method).toBe('GET');
    expect(out[0].identityKey).toBe('r1');
    // 缺少 runId 的运行被丢弃，非法状态回退为 error
    expect(out[0].runs).toHaveLength(1);
    expect(out[0].runs[0].status).toBe('error');
  });
});

describe('requests store — startRun', () => {
  it('registers a running entry immediately and selects it', () => {
    const s = useRequestsStore();
    const { requestId, runId } = s.startRun(meta());

    expect(s.entries).toHaveLength(1);
    expect(s.selectedRequestId).toBe(requestId);
    expect(s.selectedRun?.runId).toBe(runId);
    expect(s.selectedRun?.status).toBe('running');
    expect(s.isRunning(requestId)).toBe(true);
  });

  it('merges repeated runs of the same request into one entry', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    s.startRun(meta());
    s.startRun(meta());

    expect(s.entries).toHaveLength(1);
    expect(s.entries[0].runs).toHaveLength(3);
  });

  it('keeps the newest run first', () => {
    const s = useRequestsStore();
    const first = s.startRun(meta({ target: 'http://x/1' }));
    const second = s.startRun(meta({ target: 'http://x/2' }));

    expect(s.entries[0].runs[0].runId).toBe(second.runId);
    expect(s.entries[0].target).toBe('http://x/2');
    expect(first.runId).not.toBe(second.runId);
  });

  it('assigns distinct ids to different requests', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    s.startRun(meta({ identityKey: '/a.http::获取数据', name: '获取数据', method: 'GET' }));

    expect(s.entries).toHaveLength(2);
    expect(s.entries[0].requestId).not.toBe(s.entries[1].requestId);
  });

  it('caps runs per entry at 10', () => {
    const s = useRequestsStore();
    for (let i = 0; i < 14; i++) s.startRun(meta());
    expect(s.entries[0].runs).toHaveLength(10);
  });

  it('caps entries at 50', () => {
    const s = useRequestsStore();
    for (let i = 0; i < 55; i++) {
      s.startRun(meta({ identityKey: `/a.http::#${i}`, name: `请求 ${i}` }));
    }
    expect(s.entries).toHaveLength(50);
    // 最新的仍在集合中，最旧的被淘汰
    expect(s.entries[0].name).toBe('请求 54');
    expect(s.findByIdentity('/a.http::#0')).toBeNull();
  });

  it('persists to localStorage', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    expect(localStorage.getItem('http-client-pro:requests')).toContain('登录');
  });
});

describe('requests store — finish', () => {
  it('finishOk fills status and elapsed', () => {
    const s = useRequestsStore();
    const { requestId, runId } = s.startRun(meta());
    s.finishOk(requestId, runId, res(201, 33));

    const run = s.selectedRun;
    expect(run?.status).toBe('done');
    expect(run?.httpStatus).toBe(201);
    expect(run?.elapsedMs).toBe(33);
    expect(run?.response?.body).toBe('{"ok":true}');
    expect(s.isRunning(requestId)).toBe(false);
  });

  it('finishError records the message', () => {
    const s = useRequestsStore();
    const { requestId, runId } = s.startRun(meta());
    s.finishError(requestId, runId, 'timeout');

    expect(s.selectedRun?.status).toBe('error');
    expect(s.selectedRun?.error).toBe('timeout');
  });

  it('ignores finish for unknown or already finished runs', () => {
    const s = useRequestsStore();
    const { requestId, runId } = s.startRun(meta());
    s.finishOk(requestId, runId, res());
    s.finishOk(requestId, runId, res(500));
    s.finishError(requestId, runId, 'late');
    s.finishOk('nope', 'nope', res());

    expect(s.selectedRun?.httpStatus).toBe(200);
    expect(s.selectedRun?.status).toBe('done');
  });

  it('tracks parallel runs independently', () => {
    const s = useRequestsStore();
    const a = s.startRun(meta());
    const b = s.startRun(meta());
    s.finishOk(a.requestId, a.runId, res(200));

    expect(s.isRunning(a.requestId)).toBe(true); // b 仍在执行
    s.finishOk(b.requestId, b.runId, res(204));
    expect(s.isRunning(b.requestId)).toBe(false);
  });
});

describe('requests store — selection & history', () => {
  it('viewRun pins a historical run, select follows the newest', () => {
    const s = useRequestsStore();
    const first = s.startRun(meta());
    s.finishOk(first.requestId, first.runId, res(200));
    const second = s.startRun(meta());
    s.finishOk(second.requestId, second.runId, res(500));

    s.viewRun(first.requestId, first.runId);
    expect(s.selectedRun?.runId).toBe(first.runId);
    expect(s.selectedRun?.httpStatus).toBe(200);

    s.select(first.requestId);
    expect(s.selectedRun?.runId).toBe(second.runId);
  });

  it('falls back to the newest run when the pinned run is gone', () => {
    const s = useRequestsStore();
    const { requestId } = s.startRun(meta());
    s.viewRun(requestId, 'missing-run');
    expect(s.selectedRun?.runId).toBe(s.entries[0].runs[0].runId);
  });

  it('selectedEntry/selectedRun are null without selection', () => {
    const s = useRequestsStore();
    expect(s.selectedEntry).toBeNull();
    expect(s.selectedRun).toBeNull();
  });
});

describe('requests store — config', () => {
  it('updateConfig overrides the display name', () => {
    const s = useRequestsStore();
    const { requestId } = s.startRun(meta());
    s.updateConfig(requestId, { name: '  自定义名  ' });

    expect(s.entries[0].name).toBe('自定义名');
    expect(s.entries[0].nameOverridden).toBe(true);
  });

  it('empty name clears it but keeps the override flag', () => {
    const s = useRequestsStore();
    const { requestId } = s.startRun(meta());
    s.updateConfig(requestId, { name: '   ' });

    expect(s.entries[0].name).toBeNull();
    expect(s.entries[0].nameOverridden).toBe(true);
  });

  it('an overridden name no longer follows the ### name', () => {
    const s = useRequestsStore();
    const { requestId } = s.startRun(meta());
    s.updateConfig(requestId, { name: '自定义名' });
    s.startRun(meta({ name: '改名后' }));

    expect(s.entries).toHaveLength(1);
    expect(s.entries[0].name).toBe('自定义名');
  });

  it('a non-overridden name follows the ### name', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    s.startRun(meta({ name: '改名后' }));
    expect(s.entries[0].name).toBe('改名后');
  });

  it('allowParallel defaults to false and can be toggled', () => {
    const s = useRequestsStore();
    const { requestId } = s.startRun(meta());
    expect(s.entries[0].allowParallel).toBe(false);

    s.updateConfig(requestId, { allowParallel: true });
    expect(s.entries[0].allowParallel).toBe(true);

    s.updateConfig(requestId, { allowParallel: false });
    expect(s.entries[0].allowParallel).toBe(false);
  });

  it('ignores config updates for unknown requests', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    s.updateConfig('nope', { name: 'x' });
    expect(s.entries[0].name).toBe('登录');
  });
});

describe('requests store — notice / load / clear', () => {
  it('setNotice stores and startRun clears the notice', () => {
    const s = useRequestsStore();
    s.setNotice('未找到请求');
    expect(s.notice).toBe('未找到请求');
    s.startRun(meta());
    expect(s.notice).toBeNull();
  });

  it('load marks interrupted runs as errors', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    expect(s.entries[0].runs[0].status).toBe('running');

    const s2 = useRequestsStore();
    s2.load();
    expect(s2.entries).toHaveLength(1);
    expect(s2.entries[0].runs[0].status).toBe('error');
    expect(s2.entries[0].runs[0].error).toContain('Interrupted');
  });

  it('load tolerates missing storage', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    localStorage.removeItem('http-client-pro:requests');
    s.load();
    expect(s.entries).toEqual([]);
  });

  it('clear empties entries, selection and storage', () => {
    const s = useRequestsStore();
    s.startRun(meta());
    s.setNotice('x');
    s.clear();

    expect(s.entries).toEqual([]);
    expect(s.selectedRequestId).toBeNull();
    expect(s.selectedRun).toBeNull();
    expect(s.notice).toBeNull();
    expect(localStorage.getItem('http-client-pro:requests')).toBeNull();
  });
});
