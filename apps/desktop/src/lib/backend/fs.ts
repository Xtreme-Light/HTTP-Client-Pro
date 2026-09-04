import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { FileEntry } from '../../stores/workspace';

export interface TauriFs {
  listDir(path: string): Promise<FileEntry[]>;
  readFile(path: string): Promise<string>;
  writeFile(path: string, content: string): Promise<void>;
  createFile(path: string): Promise<void>;
  createDir(path: string): Promise<void>;
  renamePath(oldPath: string, newPath: string): Promise<void>;
  deleteFile(path: string): Promise<void>;
  getDefaultWorkspace(): Promise<string>;
  pickDirectory(): Promise<string | null>;
  pickFile(): Promise<string | null>;
}

let _instance: TauriFs | null = null;

function getTauri(): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  return !!(w.__TAURI_INTERNALS__ || w.__TAURI__);
}

export function getFs(): TauriFs | null {
  if (!getTauri()) return null;
  if (!_instance) {
    _instance = createTauriFs();
  }
  return _instance;
}

function createTauriFs(): TauriFs {
  return {
    async listDir(path: string): Promise<FileEntry[]> {
      const res = await invoke<{ items: FileEntry[] }>('list_dir', { path });
      return res.items;
    },

    async readFile(path: string): Promise<string> {
      return invoke<string>('read_file', { path });
    },

    async writeFile(path: string, content: string): Promise<void> {
      await invoke('write_file', { path, content });
    },

    async createFile(path: string): Promise<void> {
      await invoke('create_file', { path });
    },

    async createDir(path: string): Promise<void> {
      await invoke('create_dir', { path });
    },

    async renamePath(oldPath: string, newPath: string): Promise<void> {
      await invoke('rename_path', { oldPath, newPath });
    },

    async deleteFile(path: string): Promise<void> {
      await invoke('delete_file', { path });
    },

    async getDefaultWorkspace(): Promise<string> {
      return invoke<string>('get_default_workspace');
    },

    async pickDirectory(): Promise<string | null> {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select directory to import',
      });
      if (typeof selected === 'string' && selected.length > 0) {
        return selected;
      }
      return null;
    },

    async pickFile(): Promise<string | null> {
      const selected = await open({
        multiple: false,
        title: 'Open file',
        filters: [{ name: 'HTTP files', extensions: ['http', 'rest', 'txt'] }],
      });
      if (typeof selected === 'string' && selected.length > 0) {
        return selected;
      }
      return null;
    },
  };
}
