import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { DispatchResponse } from '../types/http';

export const useResponseStore = defineStore('response', () => {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const status = ref<number | null>(null);
  const headers = ref<Record<string, string>>({});
  const body = ref('');
  const elapsedMs = ref(0);
  const url = ref('');

  function start() {
    loading.value = true;
    error.value = null;
  }

  function ok(res: DispatchResponse) {
    loading.value = false;
    error.value = null;
    status.value = res.status;
    headers.value = res.headers;
    body.value = res.body;
    elapsedMs.value = res.elapsed_ms;
    url.value = res.url;
  }

  function fail(msg: string) {
    loading.value = false;
    error.value = msg;
  }

  function reset() {
    loading.value = false;
    error.value = null;
    status.value = null;
    headers.value = {};
    body.value = '';
    elapsedMs.value = 0;
    url.value = '';
  }

  return { loading, error, status, headers, body, elapsedMs, url, start, ok, fail, reset };
});
