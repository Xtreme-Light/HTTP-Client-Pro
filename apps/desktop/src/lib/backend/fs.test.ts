import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

// getFs 依赖 __TAURI_INTERNALS__ 才返回实例
beforeEach(() => {
  invoke.mockReset();
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
});

async function loadFs() {
  vi.resetModules();
  const mod = await import('./fs');
  return mod.getFs()!;
}

describe('fs backend path normalization', () => {
  it('normalizes backslashes in getDefaultWorkspace()', async () => {
    invoke.mockResolvedValue('C:\\Users\\me\\.http-client-pro');
    const fs = await loadFs();
    expect(await fs.getDefaultWorkspace()).toBe('C:/Users/me/.http-client-pro');
  });

  it('normalizes backslashes in listDir() item paths', async () => {
    invoke.mockResolvedValue({
      items: [{ name: 'requests.http', path: 'C:\\ws\\requests.http', isDir: false }],
    });
    const fs = await loadFs();
    const items = await fs.listDir('C:\\ws');
    expect(items[0].path).toBe('C:/ws/requests.http');
  });

  it('leaves forward-slash paths untouched on POSIX', async () => {
    invoke.mockResolvedValue('/home/me/.http-client-pro');
    const fs = await loadFs();
    expect(await fs.getDefaultWorkspace()).toBe('/home/me/.http-client-pro');
  });
});
