import { describe, expect, test } from 'bun:test';
import { workspaceIconFor } from './workspaceIcons';

describe('workspaceIconFor', () => {
  test('maps common source files to language icons and tones', () => {
    expect(workspaceIconFor({ name: 'App.tsx', kind: 'file' })).toEqual({ icon: 'file-code', tone: 'typescript' });
    expect(workspaceIconFor({ name: 'main.rs', kind: 'file' })).toEqual({ icon: 'file-code', tone: 'rust' });
    expect(workspaceIconFor({ name: 'server.py', kind: 'file' })).toEqual({ icon: 'file-code', tone: 'python' });
  });

  test('maps special project files before extension matching', () => {
    expect(workspaceIconFor({ name: 'package.json', kind: 'file' })).toEqual({ icon: 'package', tone: 'package' });
    expect(workspaceIconFor({ name: 'Cargo.toml', kind: 'file' })).toEqual({ icon: 'package', tone: 'rust' });
    expect(workspaceIconFor({ name: '.env', kind: 'file' })).toEqual({ icon: 'file-key', tone: 'security' });
    expect(workspaceIconFor({ name: 'README.md', kind: 'file' })).toEqual({ icon: 'book-open', tone: 'docs' });
  });

  test('maps data, media, archive, and binary files', () => {
    expect(workspaceIconFor({ name: 'records.sqlite', kind: 'file' })).toEqual({ icon: 'database', tone: 'data' });
    expect(workspaceIconFor({ name: 'photo.webp', kind: 'file' })).toEqual({ icon: 'file-image', tone: 'media' });
    expect(workspaceIconFor({ name: 'backup.tar.gz', kind: 'file' })).toEqual({ icon: 'file-archive', tone: 'archive' });
    expect(workspaceIconFor({ name: 'module.wasm', kind: 'file' })).toEqual({ icon: 'binary', tone: 'default' });
  });

  test('maps conventional folders to folder variants', () => {
    expect(workspaceIconFor({ name: 'src', kind: 'dir' })).toEqual({ icon: 'folder-code', tone: 'folder-code' });
    expect(workspaceIconFor({ name: 'tests', kind: 'dir' })).toEqual({ icon: 'folder-check', tone: 'folder-tests' });
    expect(workspaceIconFor({ name: '.git', kind: 'dir' })).toEqual({ icon: 'folder-git', tone: 'git' });
    expect(workspaceIconFor({ name: 'unknown-folder', kind: 'dir' }, true)).toEqual({ icon: 'folder-open', tone: 'default' });
    expect(workspaceIconFor({ name: 'unknown-folder', kind: 'dir' }, false)).toEqual({ icon: 'folder', tone: 'default' });
  });
});
