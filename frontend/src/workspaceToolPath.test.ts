import { describe, expect, test } from 'bun:test';
import { summarizeToolMessage } from './toolMessage';
import * as workspaceTools from './workspaceToolPath';

const relativePath = (path: string, root = '/srv/workspace') => workspaceTools.workspaceRelativeToolPath(path, root);

describe('chat tool workspace paths', () => {
  test('maps absolute and relative tool paths into the configured workspace', () => {
    expect(relativePath('/srv/workspace/project/src/main.ts')).toBe('project/src/main.ts');
    expect(relativePath('./project/src/main.ts')).toBe('project/src/main.ts');
    expect(relativePath('/srv/workspace')).toBe('');
    expect(relativePath('.')).toBe('');
    expect(relativePath('/project/main.ts', '/')).toBe('project/main.ts');
  });

  test('rejects outside paths, traversal and non-filesystem links without guessing', () => {
    for (const path of ['/srv/workspace-other/a.ts', '/etc/passwd', '../a.ts', 'project/../../a.ts', 'https://example.com/a.ts', 'file:///srv/workspace/a.ts', '~/a.ts', 'a\0.ts']) {
      expect(relativePath(path)).toBeNull();
    }
  });

  test('extracts file tool targets and search directories without treating other tools as file navigation', () => {
    for (const name of ['read_file', 'write_file', 'patch', 'search_files']) {
      const summary = summarizeToolMessage('{}', `functions.${name}`, JSON.stringify({ path: 'project/a.ts' }));
      expect(workspaceTools.toolWorkspacePath(summary)).toBe('project/a.ts');
    }
    const resolved = summarizeToolMessage(JSON.stringify({ resolved_path: '/srv/workspace/project/a.ts' }), 'read_file', { path: './a.ts' });
    expect(workspaceTools.toolWorkspacePath(resolved)).toBe('/srv/workspace/project/a.ts');
    expect(workspaceTools.toolWorkspacePath(summarizeToolMessage('{}', 'terminal', { path: 'project/a.ts' }))).toBeNull();
    expect(workspaceTools.toolWorkspacePath(summarizeToolMessage('{}', 'search_files', { pattern: 'needle' }))).toBeNull();
  });

  test('loads only the ancestor chain instead of scanning the workspace', () => {
    expect(workspaceTools.workspaceToolParents('project/src/main.ts')).toEqual(['', 'project', 'project/src']);
    expect(workspaceTools.workspaceToolParents('main.ts')).toEqual(['']);
    expect(workspaceTools.workspaceToolParents('')).toEqual(['']);
  });
});
