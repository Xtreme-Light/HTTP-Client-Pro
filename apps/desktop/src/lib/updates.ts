/**
 * 应用更新：检查更新 / 下载安装 / 更新日志 / 下载源。
 *
 * 更新清单（latest.json）由 CI 发布到两个下载源：
 * - 官方源 GitHub Releases：https://github.com/Xtreme-Light/HTTP-Client-Pro/releases
 * - 国内镜像 CNB：https://cnb.cool/Xtreme-Light/HTTP-Client-Pro/-/releases
 * 网络请求走 Rust 侧命令（check_update / fetch_json），规避 webview CORS 限制。
 */
import { invoke } from '@tauri-apps/api/core';

export type UpdateSource = 'github' | 'cnb';

export const UPDATE_SOURCE_LABELS: Record<UpdateSource, string> = {
  github: '官方源（GitHub）',
  cnb: 'CNB 镜像（国内下载）',
};

/** 各下载源的 Release 页面 —「打开下载页」跳转地址 */
const RELEASE_PAGES: Record<UpdateSource, string> = {
  github: 'https://github.com/Xtreme-Light/HTTP-Client-Pro/releases/latest',
  cnb: 'https://cnb.cool/Xtreme-Light/HTTP-Client-Pro/-/releases',
};

/** GitHub Releases API —「关于我们」页更新日志数据源 */
const GITHUB_RELEASES_API =
  'https://api.github.com/repos/Xtreme-Light/HTTP-Client-Pro/releases?per_page=20';

const IGNORED_VERSION_KEY = 'http-client-pro:ignored-update-version';

export interface UpdateInfo {
  /** 远端最新版本号，如 "0.2.0" */
  version: string;
  /** 当前运行版本号 */
  currentVersion: string;
  /** 更新日志（Markdown 文本） */
  notes?: string | null;
}

export interface ChangelogEntry {
  tag: string;
  name: string;
  date: string;
  body: string;
}

export function isTauri(): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  return !!(w.__TAURI_INTERNALS__ || w.__TAURI__);
}

/** 当前应用版本（非 Tauri 环境返回 "dev"） */
export async function getAppVersion(): Promise<string> {
  if (!isTauri()) return 'dev';
  const { getVersion } = await import('@tauri-apps/api/app');
  return getVersion();
}

/** 按下载源检查更新；无可用更新时返回 null */
export async function checkUpdate(source: UpdateSource): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>('check_update', { source });
}

/** 下载并安装最近一次 checkUpdate 命中的更新；完成后应用自动重启 */
export async function downloadAndInstallUpdate(): Promise<void> {
  await invoke('download_and_install_update');
}

/** 订阅下载进度事件；返回取消订阅函数 */
export async function onDownloadProgress(
  cb: (downloaded: number, total: number | null) => void,
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<{ chunk: number; total: number | null }>(
    'update://download-progress',
    (e) => cb(e.payload.chunk, e.payload.total),
  );
}

/** 打开下载页（系统默认浏览器） */
export async function openReleasePage(source: UpdateSource): Promise<void> {
  const url = RELEASE_PAGES[source];
  if (!isTauri()) {
    window.open(url, '_blank');
    return;
  }
  await invoke('plugin:shell|open', { path: url, with: null });
}

/** 拉取远端 JSON（Rust 侧代理请求）；浏览器调试环境回退为 fetch */
export async function fetchJson(url: string): Promise<unknown> {
  if (!isTauri()) {
    const res = await fetch(url, { headers: { Accept: 'application/vnd.github+json' } });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  }
  return invoke<unknown>('fetch_json', { url });
}

/** 获取各版本更新日志（GitHub Releases，最多 20 条） */
export async function fetchChangelog(): Promise<ChangelogEntry[]> {
  const data = await fetchJson(GITHUB_RELEASES_API);
  if (!Array.isArray(data)) throw new Error('unexpected response');
  return data.map((r) => {
    const rel = r as Record<string, unknown>;
    return {
      tag: String(rel.tag_name ?? ''),
      name: String(rel.name ?? rel.tag_name ?? ''),
      date: String(rel.published_at ?? '').slice(0, 10),
      body: String(rel.body ?? ''),
    };
  });
}

/* ---------------- 「忽略此版本」持久化 ---------------- */

export function getIgnoredVersion(): string {
  try {
    return localStorage.getItem(IGNORED_VERSION_KEY) ?? '';
  } catch {
    return '';
  }
}

export function setIgnoredVersion(version: string) {
  try {
    localStorage.setItem(IGNORED_VERSION_KEY, version);
  } catch {
    /* ignore */
  }
}
