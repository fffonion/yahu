import type { ToolSummary } from './toolMessage';

export function toolWorkspacePath(summary: ToolSummary): string | null {
  const name = summary.toolName.replace(/^functions\./, '');
  if (!['read_file', 'write_file', 'patch', 'search_files'].includes(name)) return null;
  const raw = summary.raw as Record<string, unknown> | null;
  const input = summary.input as Record<string, unknown> | null;
  const path = raw?.resolved_path ?? (name === 'search_files' ? input?.path ?? raw?.path : summary.filePath);
  return typeof path === 'string' && path.trim() ? path.trim() : null;
}

export function workspaceRelativeToolPath(path: string, root: string): string | null {
  if (!path || /[\x00-\x1f\\]/.test(path) || /^[a-z][a-z\d+.-]*:/i.test(path) || path.startsWith('~')) return null;
  const clean = (value: string) => value.split('/').filter(part => part && part !== '.');
  const parts = clean(path);
  if (parts.includes('..')) return null;
  if (!path.startsWith('/')) return parts.join('/');
  if (!root.startsWith('/')) return null;
  const rootParts = clean(root);
  if (rootParts.some((part, index) => parts[index] !== part)) return null;
  return parts.slice(rootParts.length).join('/');
}

export function workspaceToolParents(path: string): string[] {
  const parts = path.split('/').filter(Boolean);
  return ['', ...parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join('/'))];
}

export function workspaceRepoParents(path: string, repoPath: string): string[] | null {
  const parents = workspaceToolParents(path);
  const index = parents.indexOf(repoPath);
  return index < 0 ? null : parents.slice(index);
}

export type WorkspaceRootInfo = { name: string; path: string; index: number };

/**
 * Map a tool/file path onto the workspace path grammar. One root keeps the
 * plain relative form; several roots require the `@<index>/` prefix and only
 * paths that live under a declared root qualify.
 */
export function workspaceRootPrefix(path: string, roots: WorkspaceRootInfo[]): string | null {
  const trimmed = path.trim();
  if (!trimmed) return null;
  if (roots.length === 0) return null;
  if (roots.length === 1) return workspaceRelativeToolPath(trimmed, roots[0].path);
  if (!trimmed.startsWith('/')) {
    // Relative paths belong to the default root.
    return workspaceRelativeToolPath(trimmed, roots[0].path);
  }
  const ordered = [...roots].sort((left, right) => right.path.length - left.path.length);
  for (const root of ordered) {
    if (trimmed === root.path) return `@${root.index}`;
    if (!trimmed.startsWith(`${root.path}/`)) continue;
    const rel = workspaceRelativeToolPath(trimmed, root.path);
    if (rel === null) return null;
    return rel ? `@${root.index}/${rel}` : `@${root.index}`;
  }
  return null;
}
