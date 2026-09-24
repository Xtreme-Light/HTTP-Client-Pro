import { describe, it, expect, beforeEach } from 'vitest';
import type { FileEntry } from '../stores/workspace';
import {
  DEFAULT_FILE_SORT,
  FILE_SORT_FIELDS,
  loadFileSort,
  saveFileSort,
  sortFileEntries,
  type FileSort,
} from './fileSort';

function file(name: string, extra: Partial<FileEntry> = {}): FileEntry {
  return { name, path: `/ws/${name}`, isDir: false, ...extra };
}

function dir(name: string, extra: Partial<FileEntry> = {}): FileEntry {
  return { name, path: `/ws/${name}`, isDir: true, ...extra };
}

function names(entries: FileEntry[], sort: FileSort): string[] {
  return sortFileEntries(entries, sort).map((e) => e.name);
}

beforeEach(() => {
  localStorage.clear();
});

describe('sortFileEntries', () => {
  it('defaults to name descending so newer date-prefixed files come first', () => {
    expect(DEFAULT_FILE_SORT).toEqual({ field: 'name', order: 'desc' });
    const entries = [file('20260923.http'), file('20260924xxx.http'), file('20260901.http')];
    expect(names(entries, DEFAULT_FILE_SORT)).toEqual([
      '20260924xxx.http',
      '20260923.http',
      '20260901.http',
    ]);
  });

  it('supports name ascending', () => {
    const entries = [file('b.http'), file('a.http'), file('c.http')];
    expect(names(entries, { field: 'name', order: 'asc' })).toEqual(['a.http', 'b.http', 'c.http']);
  });

  it('compares embedded numbers numerically rather than lexicographically', () => {
    const entries = [file('req-10.http'), file('req-2.http'), file('req-1.http')];
    expect(names(entries, { field: 'name', order: 'asc' })).toEqual([
      'req-1.http',
      'req-2.http',
      'req-10.http',
    ]);
  });

  it('sorts by modified time in both directions', () => {
    const entries = [
      file('old.http', { modifiedAt: 1000 }),
      file('new.http', { modifiedAt: 3000 }),
      file('mid.http', { modifiedAt: 2000 }),
    ];
    expect(names(entries, { field: 'modified', order: 'desc' })).toEqual([
      'new.http',
      'mid.http',
      'old.http',
    ]);
    expect(names(entries, { field: 'modified', order: 'asc' })).toEqual([
      'old.http',
      'mid.http',
      'new.http',
    ]);
  });

  it('sorts by created time independently from modified time', () => {
    const entries = [
      file('a.http', { createdAt: 5000, modifiedAt: 1000 }),
      file('b.http', { createdAt: 1000, modifiedAt: 5000 }),
    ];
    expect(names(entries, { field: 'created', order: 'desc' })).toEqual(['a.http', 'b.http']);
    expect(names(entries, { field: 'modified', order: 'desc' })).toEqual(['b.http', 'a.http']);
  });

  it('falls back to name when timestamps are equal or missing', () => {
    const entries = [file('b.http'), file('a.http', { modifiedAt: 1000 }), file('c.http', { modifiedAt: 1000 })];
    expect(names(entries, { field: 'modified', order: 'desc' })).toEqual([
      'c.http',
      'a.http',
      'b.http',
    ]);
  });

  it('keeps directories before files regardless of sort direction', () => {
    const entries = [file('z.http'), dir('m-dir'), file('a.http'), dir('b-dir')];
    expect(names(entries, { field: 'name', order: 'desc' })).toEqual([
      'm-dir',
      'b-dir',
      'z.http',
      'a.http',
    ]);
    expect(names(entries, { field: 'name', order: 'asc' })).toEqual([
      'b-dir',
      'm-dir',
      'a.http',
      'z.http',
    ]);
  });

  it('does not mutate the input array', () => {
    const entries = [file('b.http'), file('a.http')];
    sortFileEntries(entries, { field: 'name', order: 'asc' });
    expect(entries.map((e) => e.name)).toEqual(['b.http', 'a.http']);
  });
});

describe('file sort persistence', () => {
  it('exposes the three common sort fields', () => {
    expect(FILE_SORT_FIELDS.map((f) => f.id)).toEqual(['name', 'modified', 'created']);
  });

  it('returns the default when nothing was saved', () => {
    expect(loadFileSort()).toEqual(DEFAULT_FILE_SORT);
  });

  it('round-trips a saved preference', () => {
    saveFileSort({ field: 'modified', order: 'asc' });
    expect(loadFileSort()).toEqual({ field: 'modified', order: 'asc' });
  });

  it('ignores invalid stored values', () => {
    localStorage.setItem('http-client-pro:file-sort', JSON.stringify({ field: 'size', order: 'sideways' }));
    expect(loadFileSort()).toEqual(DEFAULT_FILE_SORT);
  });

  it('ignores corrupt stored JSON', () => {
    localStorage.setItem('http-client-pro:file-sort', '{oops');
    expect(loadFileSort()).toEqual(DEFAULT_FILE_SORT);
  });
});
