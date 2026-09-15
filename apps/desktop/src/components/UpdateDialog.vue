<script setup lang="ts">
/**
 * 更新弹窗 — 展示新版本与更新日志，提供：
 * 取消 / 忽略此版本 / 打开下载页 / 下载并安装。
 */
import { computed, onUnmounted, ref } from 'vue';
import {
  downloadAndInstallUpdate,
  onDownloadProgress,
  setIgnoredVersion,
  type UpdateInfo,
} from '../lib/updates';

const props = defineProps<{
  info: UpdateInfo;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'open-page'): void;
}>();

const downloading = ref(false);
const downloaded = ref(0);
const total = ref<number | null>(null);
const error = ref('');

let unlisten: (() => void) | null = null;

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
});

const progressPercent = computed(() => {
  if (!total.value || total.value <= 0) return null;
  return Math.min(100, Math.round((downloaded.value / total.value) * 100));
});

const progressText = computed(() => {
  const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (progressPercent.value !== null && total.value) {
    return `${mb(downloaded.value)} / ${mb(total.value)}（${progressPercent.value}%）`;
  }
  return `${mb(downloaded.value)}…`;
});

function onCancel() {
  if (downloading.value) return;
  emit('close');
}

function onIgnore() {
  setIgnoredVersion(props.info.version);
  emit('close');
}

async function onInstall() {
  if (downloading.value) return;
  downloading.value = true;
  error.value = '';
  downloaded.value = 0;
  total.value = null;
  try {
    unlisten?.();
    unlisten = await onDownloadProgress((chunk, t) => {
      downloaded.value += chunk;
      if (t) total.value = t;
    });
    // 成功后 Rust 侧会自动重启应用，此 Promise 正常不会 resolve
    await downloadAndInstallUpdate();
  } catch (e) {
    downloading.value = false;
    error.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <div class="modal-overlay" @click="onCancel">
    <div class="modal" @click.stop>
      <h3>HTTP Client Pro v{{ info.version }} 已发布</h3>
      <p class="modal-text">当前版本为 v{{ info.currentVersion }}，是否立即更新？</p>

      <label class="notes-label">更新日志</label>
      <textarea class="notes" readonly :value="info.notes || '（无更新日志）'" />

      <div v-if="downloading" class="progress-row">
        <div class="progress-bar">
          <div
            class="progress-fill"
            :style="{ width: progressPercent !== null ? `${progressPercent}%` : '100%' }"
            :class="{ indeterminate: progressPercent === null }"
          />
        </div>
        <span class="progress-text">{{ progressText }}</span>
      </div>
      <p v-if="error" class="error-text">{{ error }}</p>

      <div class="modal-actions">
        <button class="btn-cancel" :disabled="downloading" @click="onCancel">取消</button>
        <button class="btn-cancel" :disabled="downloading" @click="onIgnore">忽略此版本</button>
        <button class="btn-cancel" :disabled="downloading" @click="$emit('open-page')">打开下载页</button>
        <button class="btn-install" :disabled="downloading" @click="onInstall">
          {{ downloading ? '下载中…' : '下载并安装' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10001;
}

.modal {
  background: var(--bg-chrome);
  border-radius: 8px;
  padding: 20px 24px;
  width: 520px;
  max-width: calc(100vw - 48px);
  box-shadow: 0 8px 24px var(--shadow);
}

.modal h3 {
  font-size: 15px;
  margin: 0 0 8px 0;
  color: var(--fg);
}

.modal-text {
  font-size: 13px;
  color: var(--fg-secondary);
  line-height: 1.6;
  margin: 0 0 12px 0;
}

.notes-label {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--fg-muted);
  margin-bottom: 4px;
}

.notes {
  width: 100%;
  height: 200px;
  box-sizing: border-box;
  resize: none;
  padding: 8px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-family: ui-monospace, monospace;
  font-size: 12px;
  line-height: 1.6;
  outline: none;
}

/* ---- 下载进度 ---- */
.progress-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}

.progress-bar {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: var(--bg-input-deep);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 3px;
  transition: width 0.2s;
}

.progress-fill.indeterminate {
  width: 30% !important;
  animation: slide 1.2s ease-in-out infinite;
}

@keyframes slide {
  0% { margin-left: -30%; }
  100% { margin-left: 100%; }
}

.progress-text {
  font-size: 12px;
  color: var(--fg-muted);
  white-space: nowrap;
}

.error-text {
  margin: 10px 0 0 0;
  font-size: 12px;
  color: var(--danger);
  word-break: break-all;
}

/* ---- 按钮 ---- */
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.modal-actions button {
  padding: 5px 14px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  cursor: pointer;
  font-size: 13px;
  transition: background 0.15s, border-color 0.15s;
}

.modal-actions button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-cancel:hover:not(:disabled) {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.btn-install {
  background: var(--success);
  color: var(--success-fg);
  border-color: var(--success);
}

.btn-install:hover:not(:disabled) {
  background: var(--success-hover);
  border-color: var(--success-hover);
}
</style>
