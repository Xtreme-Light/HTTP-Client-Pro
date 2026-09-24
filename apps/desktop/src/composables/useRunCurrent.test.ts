/**
 * useRunCurrent — 执行请求的编排逻辑测试。
 *
 * 重点覆盖「编辑器内容防抖写回 store」导致的执行时机问题：
 * 执行前必须先冲刷，否则刚编辑完点 ▶ 会拿旧源码 / 旧行号定位请求块。
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useRequestStore } from '../stores/request';
import { useRequestsStore } from '../stores/requests';
import type { BackendAdapter } from '../types/http';
import { setBackendAdapter, setSourceFlusher, useRunCurrent } from './useRunCurrent';

/** execute 收到的请求源码，按调用顺序 */
const executed: string[] = [];

function stubAdapter(): BackendAdapter {
  return {
    health: async () => true,
    async execute(source) {
      executed.push(source);
      return { status: 200, headers: {}, body: 'ok', elapsed_ms: 1, url: 'http://a' };
    },
    async *executeStream() {
      /* 测试不走流式接口 */
    },
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  executed.length = 0;
  setBackendAdapter(stubAdapter());
  setSourceFlusher(null);
});

describe('run — 执行前冲刷编辑器内容', () => {
  it('一次点击就用编辑器最新源码发起（store 里还是防抖前的旧内容）', async () => {
    const requestStore = useRequestStore();
    // 旧内容只有一个块；编辑器里已经多出一个块，但还没写回 store
    requestStore.setSource('### old\nGET http://a/old\n');
    const latest = '### old\nGET http://a/old\n\n### new\nGET http://a/new\n';
    setSourceFlusher(() => requestStore.setSource(latest));

    const { run } = useRunCurrent();
    await run(1); // 点击第 2 个块的 ▶

    expect(executed).toHaveLength(1);
    expect(executed[0]).toContain('http://a/new');
  });

  it('不冲刷时旧块列表定位不到新块 — 不发请求，只给提示', async () => {
    const requestStore = useRequestStore();
    requestStore.setSource('### old\nGET http://a/old\n');

    const { run } = useRunCurrent();
    await run(1);

    expect(executed).toHaveLength(0);
    expect(useRequestsStore().notice).toContain('未找到请求');
  });

  it('快捷键路径（不传索引）同样先冲刷再取当前块', async () => {
    const requestStore = useRequestStore();
    requestStore.setSource('GET http://a/old\n');
    setSourceFlusher(() => requestStore.setSource('GET http://a/new\n'));

    const { run } = useRunCurrent();
    await run();

    expect(executed).toHaveLength(1);
    expect(executed[0]).toContain('http://a/new');
  });
});

describe('rerun — 二次执行前冲刷编辑器内容', () => {
  it('块仍在编辑器中时用最新源码执行', async () => {
    const requestStore = useRequestStore();
    const requestsStore = useRequestsStore();
    requestStore.setSource('### named\nGET http://a/1\n');

    const { run, rerun } = useRunCurrent();
    await run(0);
    const entry = requestsStore.entries[0];
    expect(entry).toBeTruthy();

    // 模拟「改了 URL 但防抖还没写回 store」
    setSourceFlusher(() => requestStore.setSource('### named\nGET http://a/2\n'));
    await rerun(entry.requestId);

    expect(executed).toHaveLength(2);
    expect(executed[1]).toContain('http://a/2');
  });
});
