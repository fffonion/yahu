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
