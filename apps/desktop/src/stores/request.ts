import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { splitRequests, applyBlockEdit, serializeBlock, type Block, type HeaderLine } from '../lib/parse';

const DEFAULT_SOURCE = `### 登录
POST http://httpbin.org/post
Content-Type: application/json

{ "user": "admin", "pass": "secret" }

### 获取数据
GET http://httpbin.org/get?key=value
Authorization: Bearer {{token}}
`;

export const useRequestStore = defineStore('request', () => {
  const source = ref(DEFAULT_SOURCE);
  const blocks = ref<Block[]>([]);
  const cursorLine = ref(0);
  /** 当前选中的 block 索引（默认跟随光标，也可手动选中） */
  const selectedBlockIndex = ref<number | null>(null);

  function setSource(s: string) {
    source.value = s;
    blocks.value = splitRequests(s);
  }

  function setCursorLine(line: number) {
    cursorLine.value = line;
  }

  function selectBlock(index: number | null) {
    selectedBlockIndex.value = index;
  }

  /** 当前光标或手动选中的请求块 */
  const currentBlock = computed<Block | null>(() => {
    if (selectedBlockIndex.value !== null) {
      return blocks.value[selectedBlockIndex.value] ?? null;
    }
    const line = cursorLine.value;
    for (const b of blocks.value) {
      if (line >= b.startLine && line < b.endLine) return b;
    }
    return blocks.value[0] ?? null;
  });

  const currentBlockIndex = computed<number>(() => {
    if (selectedBlockIndex.value !== null) return selectedBlockIndex.value;
    const line = cursorLine.value;
    for (let idx = 0; idx < blocks.value.length; idx++) {
      const b = blocks.value[idx];
      if (line >= b.startLine && line < b.endLine) return idx;
    }
    return 0;
  });

  /** 编辑当前 block 的 method/target */
  function updateRequestLine(method: string, target: string) {
    const block = currentBlock.value;
    if (!block) return;
    const newSource = applyBlockEdit(source.value, block, { method, target });
    setSource(newSource);
  }

  /** 编辑当前 block 的 headers */
  function updateHeaders(headers: HeaderLine[]) {
    const block = currentBlock.value;
    if (!block) return;
    const newSource = applyBlockEdit(source.value, block, { headers });
    setSource(newSource);
  }

  /** 编辑当前 block 的 body */
  function updateBody(body: string) {
    const block = currentBlock.value;
    if (!block) return;
    const newSource = applyBlockEdit(source.value, block, { body });
    setSource(newSource);
  }

  /** 获取当前 block 序列化后的源码（用于执行） */
  function getBlockSource(index?: number): string | null {
    const block = index != null
      ? blocks.value[index]
      : currentBlock.value;
    if (!block) return null;
    // 重新序列化确保格式一致
    return serializeBlock(block);
  }

  // 初始解析
  blocks.value = splitRequests(source.value);

  return {
    source, blocks, cursorLine, currentBlock, currentBlockIndex, selectedBlockIndex,
    setSource, setCursorLine, selectBlock,
    updateRequestLine, updateHeaders, updateBody, getBlockSource,
  };
});
