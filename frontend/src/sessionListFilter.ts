export type SidebarSession = {
  id: string;
  source?: string;
};

export function splitSidebarSessions<T extends SidebarSession>(
  sessions: T[],
  pinnedIds: Set<string>,
  hideCronSessions = false,
) {
  const visibleSessions = hideCronSessions ? sessions.filter((session) => session.source !== 'cron') : sessions;
  return {
    pinned: visibleSessions.filter((session) => pinnedIds.has(session.id)),
    normal: visibleSessions.filter((session) => !pinnedIds.has(session.id)),
  };
}
