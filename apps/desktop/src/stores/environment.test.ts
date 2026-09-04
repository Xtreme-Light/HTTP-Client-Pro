import { describe, it, expect, beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { nextTick } from 'vue';
import { useEnvironmentStore } from './environment';

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
});

describe('environment store', () => {
  it('has default dev + staging', () => {
    const s = useEnvironmentStore();
    expect(s.envNames).toContain('dev');
    expect(s.envNames).toContain('staging');
    expect(s.current).toBe('dev');
  });

  it('switches current env', () => {
    const s = useEnvironmentStore();
    s.setCurrent('staging');
    expect(s.current).toBe('staging');
  });

  it('setVar adds to current env', () => {
    const s = useEnvironmentStore();
    s.setVar('host', 'example.com');
    expect(s.resolve('host')).toBe('example.com');
  });

  it('resolve returns undefined for unknown var', () => {
    const s = useEnvironmentStore();
    expect(s.resolve('nonexistent')).toBeUndefined();
    expect(s.isDefined('nonexistent')).toBe(false);
  });

  it('addEnv creates a new env', () => {
    const s = useEnvironmentStore();
    s.addEnv('prod');
    expect(s.envNames).toContain('prod');
  });

  it('vars are isolated per env', () => {
    const s = useEnvironmentStore();
    s.setCurrent('dev');
    s.setVar('token', 'dev-token');
    s.setCurrent('staging');
    s.setVar('token', 'staging-token');
    s.setCurrent('dev');
    expect(s.resolve('token')).toBe('dev-token');
    s.setCurrent('staging');
    expect(s.resolve('token')).toBe('staging-token');
  });

  it('deleteVar removes a variable from current env', () => {
    const s = useEnvironmentStore();
    s.setVar('token', 'abc');
    expect(s.resolve('token')).toBe('abc');
    s.deleteVar('token');
    expect(s.resolve('token')).toBeUndefined();
  });

  it('deleteVar does not throw for nonexistent var', () => {
    const s = useEnvironmentStore();
    expect(() => s.deleteVar('nope')).not.toThrow();
  });

  it('removeEnv deletes an environment', () => {
    const s = useEnvironmentStore();
    s.addEnv('prod');
    expect(s.envNames).toContain('prod');
    s.removeEnv('prod');
    expect(s.envNames).not.toContain('prod');
  });

  it('removeEnv switches current if removing the active env', () => {
    const s = useEnvironmentStore();
    s.addEnv('temp');
    s.setCurrent('temp');
    expect(s.current).toBe('temp');
    s.removeEnv('temp');
    // Should fall back to the first remaining env
    expect(s.current).not.toBe('temp');
    expect(s.envNames.length).toBeGreaterThanOrEqual(1);
  });

  it('isDefined returns true for existing var', () => {
    const s = useEnvironmentStore();
    s.setVar('key', 'val');
    expect(s.isDefined('key')).toBe(true);
  });
});

describe('environment store persistence', () => {
  it('persist() writes envs to localStorage', () => {
    const s = useEnvironmentStore();
    s.setVar('host', 'example.com');
    s.persist();
    const raw = localStorage.getItem('http-client-pro:envs');
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.dev.host).toBe('example.com');
  });

  it('persist() writes current env to localStorage', () => {
    const s = useEnvironmentStore();
    s.setCurrent('staging');
    s.persist();
    expect(localStorage.getItem('http-client-pro:current-env')).toBe('staging');
  });

  it('load() restores envs from localStorage', () => {
    const data = {
      dev: { host: 'dev.example.com', token: 'dev-tok' },
      prod: { host: 'prod.example.com' },
    };
    localStorage.setItem('http-client-pro:envs', JSON.stringify(data));
    localStorage.setItem('http-client-pro:current-env', 'prod');
    const s = useEnvironmentStore();
    s.load();
    expect(s.envs.dev.host).toBe('dev.example.com');
    expect(s.envs.prod.host).toBe('prod.example.com');
    expect(s.current).toBe('prod');
    expect(s.resolve('host')).toBe('prod.example.com');
  });

  it('load() does not throw for empty localStorage', () => {
    const s = useEnvironmentStore();
    expect(() => s.load()).not.toThrow();
    // Should keep defaults
    expect(s.envNames).toContain('dev');
  });

  it('load() handles malformed JSON gracefully', () => {
    localStorage.setItem('http-client-pro:envs', '{not valid json');
    const s = useEnvironmentStore();
    expect(() => s.load()).not.toThrow();
    // Should keep defaults
    expect(s.envNames).toContain('dev');
  });

  it('load() ignores current env that no longer exists', () => {
    const data = { dev: {} };
    localStorage.setItem('http-client-pro:envs', JSON.stringify(data));
    localStorage.setItem('http-client-pro:current-env', 'deleted-env');
    const s = useEnvironmentStore();
    s.load();
    // Should not switch to a nonexistent env
    expect(s.current).not.toBe('deleted-env');
  });

  it('auto-persists on env change via watch', async () => {
    const s = useEnvironmentStore();
    s.setVar('auto-key', 'auto-val');
    // The watch in environment.ts auto-persists on deep change — wait for it
    await nextTick();
    const raw = localStorage.getItem('http-client-pro:envs');
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.dev['auto-key']).toBe('auto-val');
  });
});
