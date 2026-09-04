import { defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';

const STORAGE_KEY = 'http-client-pro:envs';
const CURRENT_KEY = 'http-client-pro:current-env';

export const useEnvironmentStore = defineStore('environment', () => {
  const envs = ref<Record<string, Record<string, string>>>({
    dev: {},
    staging: {},
  });
  const current = ref<string | null>('dev');

  const vars = computed(() => {
    if (!current.value) return {};
    return envs.value[current.value] ?? {};
  });

  const envNames = computed(() => Object.keys(envs.value));

  function resolve(name: string): string | undefined {
    return vars.value[name];
  }

  function setCurrent(name: string) {
    if (name in envs.value) {
      current.value = name;
    }
  }

  function setVar(key: string, value: string) {
    if (!current.value) return;
    if (!envs.value[current.value]) {
      envs.value[current.value] = {};
    }
    envs.value[current.value][key] = value;
  }

  function deleteVar(key: string) {
    if (!current.value) return;
    delete envs.value[current.value]?.[key];
  }

  function addEnv(name: string) {
    if (!(name in envs.value)) {
      envs.value[name] = {};
    }
  }

  function removeEnv(name: string) {
    delete envs.value[name];
    if (current.value === name) {
      current.value = Object.keys(envs.value)[0] ?? null;
    }
  }

  function isDefined(name: string): boolean {
    return resolve(name) !== undefined;
  }

  /** 从 localStorage 加载 */
  function load() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object') {
          envs.value = parsed;
        }
      }
      const cur = localStorage.getItem(CURRENT_KEY);
      if (cur && cur in envs.value) {
        current.value = cur;
      }
    } catch { /* */ }
  }

  /** 持久化到 localStorage */
  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(envs.value));
      if (current.value) {
        localStorage.setItem(CURRENT_KEY, current.value);
      }
    } catch { /* */ }
  }

  // 自动持久化
  watch(envs, () => persist(), { deep: true });
  watch(current, () => persist());

  return {
    envs, current, vars, envNames,
    resolve, isDefined,
    setCurrent, setVar, deleteVar, addEnv, removeEnv,
    load, persist,
  };
});
