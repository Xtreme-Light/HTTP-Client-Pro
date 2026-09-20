<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useWorkspaceStore } from '../stores/workspace';
import { useSettingsStore, type SettingsSnapshot } from '../stores/settings';
import {
  EDITOR_SCHEME_FOLLOW,
  EDITOR_SCHEMES,
  UI_THEMES,
  getEditorScheme,
  type UiTheme,
} from '../lib/themes';
import { KEYMAP_SCHEME_LIST, SHORTCUT_ACTIONS, formatBinding, getKeymapScheme } from '../lib/keymaps';
import { FONT_SIZES, SYSTEM_FONTS, buildUiFontStack, isFontAvailable } from '../lib/fonts';
import {
  UPDATE_SOURCE_LABELS,
  fetchChangelog,
  getAppVersion,
  type ChangelogEntry,
  type UpdateSource,
} from '../lib/updates';

const workspaceStore = useWorkspaceStore();
const settings = useSettingsStore();

const SETTINGS_TAB_PATH = '__settings__';

type MenuKey = 'appearance' | 'editor' | 'shortcuts' | 'about';

const activeMenu = ref<MenuKey>('appearance');

const menus: { key: MenuKey; label: string }[] = [
  { key: 'appearance', label: '外观' },
  { key: 'editor', label: '编辑器' },
  { key: 'shortcuts', label: '快捷键' },
  { key: 'about', label: '关于我们' },
];

/* ---------------- 快照 / 应用 / 关闭 ---------------- */

let snapshot: SettingsSnapshot = settings.snapshot();

onMounted(() => {
  // 打开设置时快照，未应用直接关闭则恢复
  snapshot = settings.snapshot();
});

const isDirty = computed(() => {
  const cur = settings.snapshot();
  return JSON.stringify(cur) !== JSON.stringify(snapshot);
});

function onApply() {
  settings.apply();
  snapshot = settings.snapshot();
}

function onClose() {
  if (isDirty.value) {
    settings.restore(snapshot);
  }
  workspaceStore.closeTab(SETTINGS_TAB_PATH);
}

function onApplyAndClose() {
  onApply();
  workspaceStore.closeTab(SETTINGS_TAB_PATH);
}

/* ---------------- 外观：主题 ---------------- */

/** 内置主题与开源主题分组 */
const builtinThemes = computed(() => UI_THEMES.filter((t) => t.builtin));
const openSourceThemes = computed(() => UI_THEMES.filter((t) => !t.builtin));

function selectTheme(t: UiTheme) {
  settings.themeId = t.id;
}

/* ---------------- 外观：编辑器 color scheme ---------------- */

const editorSchemeOptions = computed(() => {
  const followScheme = getEditorScheme(settings.theme.editorScheme);
  return [
    { id: EDITOR_SCHEME_FOLLOW, name: `跟随主题（${followScheme.name}）` },
    ...EDITOR_SCHEMES.map((s) => ({ id: s.id, name: s.name })),
  ];
});

/** 当前生效的 scheme（用于预览） */
const currentScheme = computed(() => settings.resolvedEditorScheme);

/* ---------------- 外观：字体 ---------------- */

const systemFontNames = SYSTEM_FONTS.map((f) => f.name);

const fontOptions = computed(() =>
  SYSTEM_FONTS.map((f) => ({
    ...f,
    available: isFontAvailable(f.name),
  })),
);

const fontSelect = computed<string>({
  get() {
    return settings.uiFont === '' || systemFontNames.includes(settings.uiFont)
      ? settings.uiFont
      : '__custom__';
  },
  set(v) {
    if (v !== '__custom__') settings.uiFont = v;
  },
});

/** 预览字体栈 */
const previewFontFamily = computed(() =>
  buildUiFontStack(settings.uiFont, settings.uiFontFallback),
);

/* ---------------- 快捷键 ---------------- */

const shortcutGroups = computed(() => {
  const groups: { name: string; items: typeof SHORTCUT_ACTIONS }[] = [];
  for (const action of SHORTCUT_ACTIONS) {
    let g = groups.find((x) => x.name === action.group);
    if (!g) {
      g = { name: action.group, items: [] };
      groups.push(g);
    }
    g.items.push(action);
  }
  return groups;
});

/** 当前选中的快捷键方案 */
const currentKeymap = computed(() => getKeymapScheme(settings.keymapScheme));

function bindingParts(binding: string): string[] {
  return formatBinding(binding).split('+');
}

/* ---------------- 关于我们：版本 / 更新源 / 更新日志 ---------------- */

const appVersion = ref('');
const changelog = ref<ChangelogEntry[]>([]);
const changelogLoading = ref(false);
const changelogError = ref('');

const updateSourceOptions: { value: UpdateSource; label: string }[] = [
  { value: 'github', label: UPDATE_SOURCE_LABELS.github },
  { value: 'cnb', label: UPDATE_SOURCE_LABELS.cnb },
];

async function loadAbout() {
  appVersion.value = await getAppVersion().catch(() => '未知');
  await loadChangelog();
}

async function loadChangelog() {
  changelogLoading.value = true;
  changelogError.value = '';
  try {
    changelog.value = await fetchChangelog();
  } catch (e) {
    changelogError.value = e instanceof Error ? e.message : String(e);
  } finally {
    changelogLoading.value = false;
  }
}

onMounted(loadAbout);
</script>

<template>
  <div class="settings-pane">
    <div class="settings-layout">
      <!-- 左侧菜单 -->
      <div class="settings-menu">
        <button
          v-for="m in menus"
          :key="m.key"
          class="settings-menu-item"
          :class="{ active: activeMenu === m.key }"
          @click="activeMenu = m.key"
        >
          {{ m.label }}
        </button>
      </div>

      <!-- 右侧内容 -->
      <div class="settings-content">
        <!-- ============ 外观 ============ -->
        <div v-if="activeMenu === 'appearance'" class="settings-section">
          <h3>外观</h3>

          <div class="settings-grid">
            <!-- 主题 -->
            <div class="field col-6">
              <label class="field-label">主题</label>
              <select v-model="settings.themeId" class="select">
                <optgroup label="基础主题">
                  <option v-for="t in builtinThemes" :key="t.id" :value="t.id">{{ t.name }}</option>
                </optgroup>
                <optgroup label="开源主题">
                  <option v-for="t in openSourceThemes" :key="t.id" :value="t.id">{{ t.name }}</option>
                </optgroup>
              </select>
            </div>

            <!-- 编辑器 color scheme -->
            <div class="field col-6">
              <label class="field-label">编辑器颜色方案（Color Scheme）</label>
              <select v-model="settings.editorSchemeId" class="select">
                <option v-for="s in editorSchemeOptions" :key="s.id" :value="s.id">{{ s.name }}</option>
              </select>
              <div class="scheme-preview" :style="{ background: currentScheme.background }">
                <span :style="{ color: currentScheme.syntax.method, fontWeight: 'bold' }">POST</span>
                <span :style="{ color: currentScheme.syntax.url }">https://api.example.com/users</span>
                <span :style="{ color: currentScheme.syntax.headerName }">Content-Type</span><span :style="{ color: currentScheme.syntax.separator }">: </span><span :style="{ color: currentScheme.syntax.headerValue }">application/json</span>
                <span :style="{ color: currentScheme.syntax.comment }"># {{ currentScheme.name }}</span>
                <span
                  class="scheme-body"
                  :style="{ background: currentScheme.syntax.bodyBg, color: currentScheme.syntax.body }"
                >{"id": 1}</span>
              </div>
              <p class="field-hint">
                选择「跟随主题」时，编辑器配色随 UI 主题自动切换；也可独立指定。
              </p>
            </div>

            <!-- 卡片式主题预览 -->
            <div class="field col-12">
              <label class="field-label">主题预览</label>
              <div class="theme-grid">
                <button
                  v-for="t in UI_THEMES"
                  :key="t.id"
                  class="theme-card"
                  :class="{ active: settings.themeId === t.id }"
                  @click="selectTheme(t)"
                >
                  <div class="theme-preview" :style="{ background: t.vars['bg-base'] }">
                    <div
                      class="tp-title"
                      :style="{
                        background: t.vars['bg-chrome'],
                        borderBottom: `1px solid ${t.vars['border']}`,
                        color: t.vars['fg-muted'],
                      }"
                    >
                      HTTP Client Pro
                    </div>
                    <div class="tp-body">
                      <div class="tp-sidebar" :style="{ background: t.vars['bg-panel'] }">
                        <div
                          class="tp-item"
                          :style="{ background: t.vars['bg-active'], color: t.vars['fg-strong'] }"
                        >
                          request.http
                        </div>
                        <div class="tp-item" :style="{ color: t.vars['fg-muted'] }">env.json</div>
                      </div>
                      <div class="tp-editor">
                        <div class="tp-line">
                          <span :style="{ color: t.vars['syn-keyword'], fontWeight: 'bold' }">GET</span>
                          <span :style="{ color: t.vars['syn-url'] }">https://api.example.com</span>
                        </div>
                        <div class="tp-line" :style="{ color: t.vars['syn-comment'] }"># 请求头</div>
                        <div class="tp-line">
                          <span :style="{ color: t.vars['syn-keyword'] }">Accept</span><span :style="{ color: t.vars['fg-muted'] }">: </span><span :style="{ color: t.vars['syn-string'] }">application/json</span>
                        </div>
                      </div>
                    </div>
                  </div>
                  <div class="theme-meta">
                    <span class="theme-name">{{ t.name }}</span>
                    <span class="theme-source">{{ t.source }}</span>
                  </div>
                </button>
              </div>
            </div>

            <!-- 字体 -->
            <div class="field col-12">
              <label class="field-label">界面字体</label>
              <div class="font-row">
                <select v-model="fontSelect" class="select">
                  <option value="">跟随系统默认</option>
                  <option
                    v-for="f in fontOptions"
                    :key="f.name"
                    :value="f.name"
                  >
                    {{ f.name }}（{{ f.hint }}{{ f.available ? '' : ' · 未检测到' }}）
                  </option>
                  <option v-if="fontSelect === '__custom__'" value="__custom__">自定义：{{ settings.uiFont }}</option>
                </select>
                <input
                  v-model="settings.uiFont"
                  class="input"
                  type="text"
                  placeholder="自定义字体名称，如 Inter"
                />
              </div>
              <p class="field-hint">作用于全部界面文字（含 HISTORY、Console）与 Editor；「跟随系统默认」时 Editor 使用内置等宽字体。</p>
            </div>

            <div class="field col-6">
              <label class="field-label">字体大小</label>
              <select v-model="settings.fontSize" class="select">
                <option v-for="s in FONT_SIZES" :key="s" :value="s">{{ s }} px</option>
              </select>
              <p class="field-hint">以 13px 为基准等比缩放全站文字（界面、Editor、HISTORY、Console）。</p>
            </div>

            <div class="field col-6">
              <label class="field-label">字体回退（Fallback）</label>
              <input
                v-model="settings.uiFontFallback"
                class="input"
                type="text"
                placeholder="如：Arial, 'Noto Sans SC', sans-serif"
              />
              <p class="field-hint">主字体缺失时按顺序回退；留空则以系统无衬线字体兜底。</p>
            </div>

            <div class="field col-12">
              <label class="field-label">字体预览</label>
              <div class="font-preview" :style="{ fontFamily: previewFontFamily }">
                <div class="font-preview-main">HTTP Client Pro 界面字体预览 The quick brown fox jumps over the lazy dog.</div>
                <div class="font-preview-sub">0123456789 — {{ previewFontFamily }}</div>
              </div>
            </div>
          </div>
        </div>

        <!-- ============ 编辑器 ============ -->
        <div v-else-if="activeMenu === 'editor'" class="settings-section">
          <h3>编辑器</h3>

          <div class="settings-grid">
            <div class="field col-6">
              <label class="field-label">行折叠（长行折行）</label>
              <select v-model="settings.editorLineWrap" class="select">
                <option :value="true">开启：超出宽度的内容折行显示</option>
                <option :value="false">关闭：单行显示，横向滚动查看</option>
              </select>
              <p class="field-hint">默认开启。一行内容过长时自动换到下一行展示，不改变文件本身的内容。</p>
            </div>
          </div>
        </div>

        <!-- ============ 快捷键 ============ -->
        <div v-else-if="activeMenu === 'shortcuts'" class="settings-section">
          <h3>快捷键</h3>

          <div class="settings-grid">
            <div class="field col-6">
              <label class="field-label">快捷键方案</label>
              <select v-model="settings.keymapScheme" class="select">
                <option v-for="s in KEYMAP_SCHEME_LIST" :key="s.id" :value="s.id">
                  {{ s.name }}
                </option>
              </select>
              <p class="field-hint">{{ currentKeymap.description }}</p>
            </div>
          </div>

          <div class="shortcut-groups">
            <div v-for="g in shortcutGroups" :key="g.name" class="shortcut-group">
              <div class="shortcut-group-title">{{ g.name }}</div>
              <table class="shortcut-table">
                <tbody>
                  <tr v-for="a in g.items" :key="a.id">
                    <td class="shortcut-label">{{ a.label }}</td>
                    <td class="shortcut-keys">
                      <template v-for="(p, i) in bindingParts(currentKeymap.bindings[a.id])" :key="i">
                        <kbd>{{ p }}</kbd>
                        <span v-if="i < bindingParts(currentKeymap.bindings[a.id]).length - 1" class="plus">+</span>
                      </template>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <!-- ============ 关于 ============ -->
        <div v-else-if="activeMenu === 'about'" class="settings-section">
          <h3>关于我们</h3>

          <div class="settings-grid">
            <div class="field col-6">
              <label class="field-label">应用信息</label>
              <div class="about-app">
                <div class="about-name">HTTP Client Pro</div>
                <div class="about-version">版本 v{{ appVersion || '…' }}</div>
              </div>
            </div>

            <div class="field col-6">
              <label class="field-label">更新下载源</label>
              <select v-model="settings.updateSource" class="select">
                <option v-for="o in updateSourceOptions" :key="o.value" :value="o.value">
                  {{ o.label }}
                </option>
              </select>
              <p class="field-hint">
                国内下载：如果 GitHub 下载较慢，可切换为 CNB 镜像下载桌面端安装包。
              </p>
            </div>

            <div class="field col-12">
              <div class="changelog-header">
                <label class="field-label">更新日志</label>
                <button class="changelog-refresh" :disabled="changelogLoading" @click="loadChangelog">
                  {{ changelogLoading ? '加载中…' : '刷新' }}
                </button>
              </div>
              <div class="changelog-list">
                <p v-if="changelogError" class="changelog-empty">
                  更新日志加载失败：{{ changelogError }}
                </p>
                <p v-else-if="!changelogLoading && changelog.length === 0" class="changelog-empty">
                  暂无发布版本
                </p>
                <div v-for="entry in changelog" :key="entry.tag" class="changelog-entry">
                  <div class="changelog-entry-title">
                    <span class="changelog-tag">{{ entry.tag }}</span>
                    <span class="changelog-name">{{ entry.name }}</span>
                    <span v-if="entry.date" class="changelog-date">{{ entry.date }}</span>
                  </div>
                  <pre class="changelog-body">{{ entry.body || '（无更新说明）' }}</pre>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部按钮 -->
    <div class="settings-actions">
      <span v-if="isDirty" class="dirty-hint">有未应用的更改</span>
      <button class="btn-close" @click="onClose">关闭</button>
      <button class="btn-apply" :disabled="!isDirty" @click="onApply">应用</button>
      <button class="btn-apply-close" :disabled="!isDirty" @click="onApplyAndClose">应用并关闭</button>
    </div>
  </div>
</template>

<style scoped>
.settings-pane {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
  overflow: hidden;
}

.settings-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.settings-menu {
  width: 160px;
  min-width: 160px;
  background: var(--bg-panel);
  border-right: 1px solid var(--border);
  padding: 8px 0;
  overflow-y: auto;
}

.settings-menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 6px 16px;
  border: none;
  background: none;
  color: var(--fg-secondary);
  font-size: calc(13px * var(--font-scale, 1));
  cursor: pointer;
  transition: background 0.15s;
}

.settings-menu-item:hover {
  background: var(--bg-hover);
}

.settings-menu-item.active {
  background: var(--bg-active);
  color: var(--fg-strong);
  border-left: 2px solid var(--accent);
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
}

.settings-section {
  max-width: 1080px;
}

.settings-section h3 {
  font-size: calc(16px * var(--font-scale, 1));
  margin: 0 0 16px 0;
  color: var(--fg);
}

/* ---------- 12 栏栅格布局 ---------- */
.settings-grid {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  column-gap: 16px;
  row-gap: 18px;
}

.col-6 {
  grid-column: span 6;
}

.col-12 {
  grid-column: span 12;
}

/* 窄窗口时半栏字段堆叠为整行 */
@media (max-width: 860px) {
  .col-6 {
    grid-column: span 12;
  }
}

/* ---------- 字段 ---------- */
.field {
  min-width: 0;
}

.field-label {
  display: block;
  font-size: calc(13px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg-secondary);
  margin-bottom: 6px;
}

.field-hint {
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-muted);
  margin-top: 6px;
}

/* 下拉框与输入框统一尺寸：高 30px、圆角 4px */
.select,
.input {
  width: 100%;
  height: 30px;
  box-sizing: border-box;
  padding: 0 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: calc(13px * var(--font-scale, 1));
  line-height: 28px;
  outline: none;
}

.select:focus,
.input:focus {
  border-color: var(--focus);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

/* 界面字体：下拉框与输入框各占 50% */
.font-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 16px;
}

/* ---------- 主题卡片：最少 2 列，最多 4 列 ---------- */
.theme-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

@media (min-width: 1100px) {
  .theme-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (min-width: 1400px) {
  .theme-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

.theme-card {
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: var(--bg-panel);
  padding: 8px;
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.theme-card:hover {
  border-color: var(--fg-muted);
}

.theme-card.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.theme-preview {
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid var(--border-block);
  height: 96px;
  display: flex;
  flex-direction: column;
}

.tp-title {
  font-size: calc(9px * var(--font-scale, 1));
  padding: 2px 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tp-body {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.tp-sidebar {
  width: 38%;
  padding: 3px 0;
  flex-shrink: 0;
}

.tp-item {
  font-size: calc(8px * var(--font-scale, 1));
  padding: 1px 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tp-editor {
  flex: 1;
  padding: 3px 6px;
  overflow: hidden;
}

.tp-line {
  font-size: calc(8px * var(--font-scale, 1));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.7;
}

.theme-meta {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 6px;
  margin-top: 6px;
}

.theme-name {
  font-size: calc(12px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg);
  white-space: nowrap;
}

.theme-source {
  font-size: calc(11px * var(--font-scale, 1));
  color: var(--fg-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ---------- scheme 预览 ---------- */
.scheme-preview {
  margin-top: 6px;
  padding: 6px 10px;
  border-radius: 4px;
  border: 1px solid var(--border-block);
  font-family: var(--font-editor);
  font-size: calc(12px * var(--font-scale, 1));
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  white-space: nowrap;
  overflow: hidden;
}

/* 预览请求体底色（对应编辑器 .cm-body-line） */
.scheme-body {
  padding: 0 6px;
  border-radius: 3px;
}

/* ---------- 字体预览 ---------- */
.font-preview {
  border: 1px solid var(--border-block);
  border-radius: 4px;
  background: var(--bg-input-deep);
  padding: 10px 12px;
}

.font-preview-main {
  font-size: calc(14px * var(--font-scale, 1));
  color: var(--fg);
  margin-bottom: 4px;
}

.font-preview-sub {
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-muted);
}

/* ---------- 快捷键表格 ---------- */
.shortcut-groups {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 14px;
  margin-top: 18px;
}

.shortcut-group-title {
  font-size: calc(12px * var(--font-scale, 1));
  font-weight: 700;
  color: var(--fg-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 6px;
}

.shortcut-table {
  width: 100%;
  border-collapse: collapse;
}

.shortcut-table td {
  padding: 5px 8px;
  font-size: calc(13px * var(--font-scale, 1));
  border-bottom: 1px solid var(--border);
}

.shortcut-label {
  color: var(--fg-secondary);
}

.shortcut-keys {
  text-align: right;
  white-space: nowrap;
}

.shortcut-keys kbd {
  display: inline-block;
  padding: 2px 7px;
  border: 1px solid var(--border-strong);
  border-bottom-width: 2px;
  border-radius: 4px;
  background: var(--bg-input);
  color: var(--fg);
  font-size: calc(11px * var(--font-scale, 1));
  font-family: var(--font-editor);
}

.shortcut-keys .plus {
  color: var(--fg-muted);
  margin: 0 2px;
}

/* ---------- 关于我们 ---------- */
.about-app {
  border: 1px solid var(--border-block);
  border-radius: 4px;
  background: var(--bg-panel);
  padding: 8px 12px;
}

.about-name {
  font-size: calc(14px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg);
}

.about-version {
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-muted);
  margin-top: 2px;
}

.changelog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.changelog-header .field-label {
  margin-bottom: 0;
}

.changelog-refresh {
  height: 24px;
  padding: 0 12px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  font-size: calc(12px * var(--font-scale, 1));
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.changelog-refresh:hover:not(:disabled) {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.changelog-refresh:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.changelog-list {
  border: 1px solid var(--border-block);
  border-radius: 4px;
  background: var(--bg-input-deep);
  max-height: 420px;
  overflow-y: auto;
  padding: 8px 12px;
}

.changelog-empty {
  font-size: calc(13px * var(--font-scale, 1));
  color: var(--fg-muted);
  margin: 4px 0;
}

.changelog-entry + .changelog-entry {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

.changelog-entry-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.changelog-tag {
  font-size: calc(13px * var(--font-scale, 1));
  font-weight: 700;
  color: var(--accent);
  font-family: var(--font-editor);
}

.changelog-name {
  font-size: calc(13px * var(--font-scale, 1));
  font-weight: 600;
  color: var(--fg);
}

.changelog-date {
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-muted);
  margin-left: auto;
}

.changelog-body {
  margin: 6px 0 0 0;
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--fg-secondary);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: inherit;
}

/* ---------- 底部按钮 ---------- */
.settings-actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-top: 1px solid var(--border);
  background: var(--bg-panel);
  flex-shrink: 0;
}

.dirty-hint {
  margin-right: auto;
  font-size: calc(12px * var(--font-scale, 1));
  color: var(--warning);
}

.settings-actions button {
  height: 30px;
  box-sizing: border-box;
  padding: 0 14px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg-button);
  color: var(--fg-secondary);
  cursor: pointer;
  font-size: calc(13px * var(--font-scale, 1));
  transition: background 0.15s, border-color 0.15s;
}

.settings-actions button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-close:hover:not(:disabled) {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.btn-apply:hover:not(:disabled) {
  background: var(--bg-button-hover);
  border-color: var(--bg-button-hover);
}

.btn-apply-close {
  background: var(--success);
  color: var(--success-fg);
  border-color: var(--success);
}

.btn-apply-close:hover:not(:disabled) {
  background: var(--success-hover);
  border-color: var(--success-hover);
}
</style>
