export type SidebarSession = {
  id: string;
  source?: string;
};

export function splitSidebarSessions<T extends SidebarSession>(
  sessions: T[],
  pinnedIds: Set<string>,
) {
  return {
    pinned: sessions.filter((session) => pinnedIds.has(session.id)),
    normal: sessions.filter((session) => !pinnedIds.has(session.id)),
  };
}
