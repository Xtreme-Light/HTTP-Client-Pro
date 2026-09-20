<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { EXAMPLES } from '../lib/examples';

// ---- emit & props 声明（必须在使用前定义）----
const emit = defineEmits<{
  (e: 'toggle-sidebar'): void;
  (e: 'open-settings'): void;
  (e: 'check-updates'): void;
  (e: 'open-example', id: string): void;
}>();

const props = defineProps<{
  sidebarVisible?: boolean;
}>();

const appWindow = getCurrentWindow();
const isMaximized = ref(false);

// ---- 请求示例下拉菜单 ----
const exampleMenuOpen = ref(false);
const exampleMenuEl = ref<HTMLElement>();

function toggleExampleMenu() {
  exampleMenuOpen.value = !exampleMenuOpen.value;
}

function openExample(id: string) {
  exampleMenuOpen.value = false;
  emit('open-example', id);
}

function onDocMouseDown(e: MouseEvent) {
  if (exampleMenuEl.value && e.target instanceof Node && !exampleMenuEl.value.contains(e.target)) {
    exampleMenuOpen.value = false;
  }
}

watch(exampleMenuOpen, (open) => {
  if (open) document.addEventListener('mousedown', onDocMouseDown, true);
  else document.removeEventListener('mousedown', onDocMouseDown, true);
});

// ---- 窗口控制 ----
async function minimize() {
  await appWindow.minimize();
}
async function toggleMaximize() {
  await appWindow.toggleMaximize();
  isMaximized.value = await appWindow.isMaximized();
}
async function close() {
  await appWindow.close();
}

onMounted(() => {
  appWindow.isMaximized().then(v => { isMaximized.value = v; });
});

onUnmounted(() => {
  document.removeEventListener('mousedown', onDocMouseDown, true);
});
</script>

<template>
  <div class="titlebar" data-tauri-drag-region>
    <!-- 左侧：工作区开关 -->
    <div class="menu-bar">
      <button
        class="workspace-toggle"
        :class="{ active: props.sidebarVisible }"
        :title="props.sidebarVisible ? 'Hide workspace' : 'Show workspace'"
        @click="emit('toggle-sidebar')"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
          <path d="M2 2h5v12H2zm7 0h5v12H9z" opacity=".3"/>
          <path d="M2 2h5v12H2V2zm1 1v10h3V3H3zm6-1h5v12H9V2zm1 1v10h3V3h-3z"/>
        </svg>
        <span>Workspace</span>
      </button>
    </div>

    <!-- 右侧：检查更新 + 设置 + 窗口控制 -->
    <div class="titlebar-right">
      <!-- 检查更新按钮 -->
      <button
        class="titlebar-btn"
        title="检查更新"
        @click="emit('check-updates')"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" />
          <path d="M13.5 1.5v3h-3" />
          <path d="M8 5.8v3l1.8 1.2" />
        </svg>
      </button>

      <!-- 请求示例按钮：下拉菜单，点击条目以只读标签页打开 -->
      <div ref="exampleMenuEl" class="example-menu-wrap">
        <button
          class="titlebar-btn"
          title="请求示例"
          :class="{ active: exampleMenuOpen }"
          @click="toggleExampleMenu"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 2.5h7L13 5.5v8a.5.5 0 0 1-.5.5h-9a.5.5 0 0 1-.5-.5v-11a.5.5 0 0 1 .5-.5z" />
            <path d="M10 2.5V6h3" />
            <path d="M5.5 9h5M5.5 11.5h3.5" />
          </svg>
        </button>
        <div v-if="exampleMenuOpen" class="example-menu">
          <div class="example-menu-title">请求示例（只读）</div>
          <button
            v-for="ex in EXAMPLES"
            :key="ex.id"
            class="example-menu-item"
            @click="openExample(ex.id)"
          >
            {{ ex.title }}
          </button>
        </div>
      </div>

      <!-- 设置按钮：直接打开设置标签页 -->
      <button
        class="titlebar-btn"
        title="Settings"
        @click="emit('open-settings')"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 1.5a1.5 1.5 0 0 1 1.5 1.5v.5l.4.2.4-.4a1.5 1.5 0 1 1 2.1 2.1l-.4.4.2.4h.5a1.5 1.5 0 0 1 0 3h-.5l-.2.4.4.4a1.5 1.5 0 1 1-2.1 2.1l-.4-.4-.4.2v.5a1.5 1.5 0 0 1-3 0v-.5l-.4-.2-.4.4a1.5 1.5 0 1 1-2.1-2.1l.4-.4-.2-.4H4a1.5 1.5 0 0 1 0-3h.5l.2-.4-.4-.4a1.5 1.5 0 1 1 2.1-2.1l.4.4.4-.2V3A1.5 1.5 0 0 1 8 1.5zm0 4.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"/>
        </svg>
      </button>

      <!-- 最小化 -->
      <button class="titlebar-btn" title="Minimize" @click="minimize">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="2" y1="6" x2="10" y2="6" />
        </svg>
      </button>

      <!-- 最大化/还原 -->
      <button class="titlebar-btn" :title="isMaximized ? 'Restore' : 'Maximize'" @click="toggleMaximize">
        <svg v-if="!isMaximized" width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
          <rect x="2" y="2" width="8" height="8" rx="1" />
        </svg>
        <svg v-else width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
          <rect x="2" y="4" width="7" height="7" rx="1" />
          <path d="M4 2h6v6" />
        </svg>
      </button>

      <!-- 关闭 -->
      <button class="titlebar-btn close-btn" title="Close" @click="close">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="2" y1="2" x2="10" y2="10" />
          <line x1="10" y1="2" x2="2" y2="10" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 36px;
  background: var(--bg-chrome);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  user-select: none;
  -webkit-user-select: none;
}

/* ---- 左侧菜单栏 ---- */
.menu-bar {
  display: flex;
  align-items: center;
  gap: 0;
}

.workspace-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px 4px 12px;
  border: none;
  background: transparent;
  color: var(--fg-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  border-radius: 4px;
  transition: background 0.15s;
}

.workspace-toggle:hover {
  background: var(--bg-button);
}

.workspace-toggle.active {
  color: var(--accent);
}

/* ---- 右侧区域 ---- */
.titlebar-right {
  display: flex;
  align-items: center;
  gap: 0;
}

.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  background: transparent;
  color: var(--fg-secondary);
  cursor: pointer;
  transition: background 0.15s;
}

.titlebar-btn:hover {
  background: var(--bg-button);
}

.titlebar-btn.active {
  background: var(--bg-button);
}

.close-btn:hover {
  background: var(--danger-strong);
  color: var(--fg-strong);
}

/* ---- 请求示例下拉菜单 ---- */
.example-menu-wrap {
  position: relative;
}

.example-menu {
  position: absolute;
  top: 36px;
  right: 0;
  min-width: 240px;
  background: var(--bg-chrome);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 4px 12px var(--shadow);
  padding: 4px 0;
  z-index: 10000;
}

.example-menu-title {
  padding: 4px 12px 6px;
  font-size: 11px;
  color: var(--fg-muted);
  user-select: none;
}

.example-menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 6px 12px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--fg);
}

.example-menu-item:hover {
  background: var(--bg-hover);
}
</style>
