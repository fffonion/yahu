export type WorkspaceEntryKind = 'file' | 'dir' | string;
export type WorkspaceEntryClickAction = 'open' | 'none';

export function workspaceEntryClickAction(kind: WorkspaceEntryKind): WorkspaceEntryClickAction {
  return kind === 'file' || kind === 'dir' ? 'open' : 'none';
}
