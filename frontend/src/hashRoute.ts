export type HashRoute =
  | { mode: 'chat'; sessionId?: string }
  | { mode: 'cron'; jobId?: string }
  | { mode: 'images'; imageFilename?: string }
  | { mode: 'workspace'; workspaceKind?: 'file' | 'folder'; workspacePath?: string }
  | { mode: 'skills'; skillName?: string }
  | { mode: 'memory' }
  | { mode: 'settings' };

const decodePart = (value = '') => {
  try { return decodeURIComponent(value); }
  catch { return value; }
};

const encodePart = (value: string) => encodeURIComponent(value);

export function parseHashRoute(hash: string): HashRoute {
  const raw = (hash || '').replace(/^#\/?/, '').replace(/^\//, '');
  const [mode, kind, rest] = raw.split('/');
  if (mode === 'chat') return { mode: 'chat', sessionId: kind ? decodePart(kind) : undefined };
  if (mode === 'cron') return { mode: 'cron', jobId: kind ? decodePart(kind) : undefined };
  if (mode === 'images') return { mode: 'images', imageFilename: kind ? decodePart(kind) : undefined };
  if (mode === 'workspace') {
    if (kind === 'file' || kind === 'folder') return { mode: 'workspace', workspaceKind: kind, workspacePath: rest ? decodePart(rest) : '' };
    return { mode: 'workspace' };
  }
  if (mode === 'memory') return { mode: 'memory' };
  if (mode === 'skills') {
    if (kind) return { mode: 'skills', skillName: decodePart(kind) };
    return { mode: 'skills' };
  }
  if (mode === 'settings') return { mode: 'settings' };
  return { mode: 'chat' };
}

export function getCurrentHashRoute(): HashRoute {
  return parseHashRoute(globalThis.location?.hash || '');
}

export function buildHashRoute(route: HashRoute): string {
  if (route.mode === 'chat') return route.sessionId ? `#/chat/${encodePart(route.sessionId)}` : '#/chat';
  if (route.mode === 'cron') return route.jobId ? `#/cron/${encodePart(route.jobId)}` : '#/cron';
  if (route.mode === 'images') return route.imageFilename ? `#/images/${encodePart(route.imageFilename)}` : '#/images';
  if (route.mode === 'workspace') {
    if (route.workspaceKind && route.workspacePath !== undefined) return `#/workspace/${route.workspaceKind}/${encodePart(route.workspacePath)}`;
    return '#/workspace';
  }
  if (route.mode === 'skills') return route.skillName ? `#/skills/${encodePart(route.skillName)}` : '#/skills';
  return `#/${route.mode}`;
}
