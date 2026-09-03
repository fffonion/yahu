export type SidebarSession = {
  id: string;
  source?: string;
};

export function splitSidebarSessions<T extends SidebarSession>(
  sessions: T[],
  pinnedIds: Set<string>,
) {
  const sessionsById = new Map(sessions.map((session) => [session.id, session]));
  return {
    pinned: Array.from(pinnedIds).map((id) => sessionsById.get(id)).filter((session): session is T => Boolean(session)),
    normal: sessions.filter((session) => !pinnedIds.has(session.id)),
  };
}

export function reorderPinnedIds(pinnedIds: Set<string>, sourceId: string, targetId: string) {
  const order = Array.from(pinnedIds);
  const sourceIndex = order.indexOf(sourceId);
  const targetIndex = order.indexOf(targetId);
  if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex) return new Set(order);
  order.splice(sourceIndex, 1);
  order.splice(order.indexOf(targetId), 0, sourceId);
  return new Set(order);
}
