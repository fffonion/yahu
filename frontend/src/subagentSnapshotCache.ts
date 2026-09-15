import type { GoalMilestone, PersistentGoal, SubagentActivity, SubagentProgress, SubagentProgressSnapshot, SubagentTodo } from './subagentProgress';

export const SUBAGENT_SNAPSHOT_CACHE_TTL_MS = 5 * 60 * 1000;

type CachedSubagentSnapshot = {
  snapshot: SubagentProgressSnapshot;
  cachedAt: number;
};

const snapshotCache = new Map<string, CachedSubagentSnapshot>();

export function readCachedSubagentSnapshot(sessionId: string, now = Date.now()): SubagentProgressSnapshot | null {
  if (!sessionId) return null;
  const entry = snapshotCache.get(sessionId);
  if (!entry || now - entry.cachedAt >= SUBAGENT_SNAPSHOT_CACHE_TTL_MS) return null;
  return entry.snapshot;
}

export function writeCachedSubagentSnapshot(snapshot: SubagentProgressSnapshot, now = Date.now()): void {
  if (!snapshot.sessionId) return;
  snapshotCache.set(snapshot.sessionId, { snapshot, cachedAt: now });
}

export function clearSubagentSnapshotCache(): void {
  snapshotCache.clear();
}

export function sameSubagentSnapshot(previous: SubagentProgressSnapshot, next: SubagentProgressSnapshot): boolean {
  if (previous.sessionId !== next.sessionId || previous.error !== next.error) return false;
  if (!sameGoal(previous.goal, next.goal) || previous.subagents.length !== next.subagents.length) return false;
  return previous.subagents.every((item, index) => sameSubagent(item, next.subagents[index]));
}

function sameGoal(previous: PersistentGoal | undefined, next: PersistentGoal | undefined): boolean {
  if (!previous || !next) return previous === next;
  return previous.text === next.text
    && previous.status === next.status
    && previous.createdAt === next.createdAt
    && previous.lastTurnAt === next.lastTurnAt
    && previous.turnsUsed === next.turnsUsed
    && previous.maxTurns === next.maxTurns
    && sameStringArray(previous.subgoals, next.subgoals)
    && sameTodoArray(previous.todos, next.todos)
    && sameMilestoneArray(previous.milestones, next.milestones)
    && previous.lastReason === next.lastReason
    && previous.pausedReason === next.pausedReason;
}

function sameSubagent(previous: SubagentProgress, next: SubagentProgress): boolean {
  return previous.sessionId === next.sessionId
    && previous.parentSessionId === next.parentSessionId
    && previous.ancestryOmitted === next.ancestryOmitted
    && previous.task === next.task
    && previous.context === next.context
    && previous.model === next.model
    && previous.status === next.status
    && previous.startedAt === next.startedAt
    && previous.endedAt === next.endedAt
    && previous.messageCount === next.messageCount
    && previous.toolCount === next.toolCount
    && previous.apiCalls === next.apiCalls
    && previous.currentTool === next.currentTool
    && previous.summary === next.summary
    && sameTodoArray(previous.todos, next.todos)
    && sameActivityArray(previous.activity, next.activity);
}

function sameStringArray(previous: string[], next: string[]): boolean {
  return previous.length === next.length && previous.every((value, index) => value === next[index]);
}

function sameTodoArray(previous: SubagentTodo[], next: SubagentTodo[]): boolean {
  return previous.length === next.length && previous.every((item, index) => {
    const other = next[index];
    return item.id === other.id && item.content === other.content && item.status === other.status;
  });
}

function sameMilestoneArray(previous: GoalMilestone[], next: GoalMilestone[]): boolean {
  return previous.length === next.length && previous.every((item, index) => {
    const other = next[index];
    return item.turn === other.turn
      && item.timestamp === other.timestamp
      && item.verdict === other.verdict
      && item.reason === other.reason;
  });
}

function sameActivityArray(previous: SubagentActivity[], next: SubagentActivity[]): boolean {
  return previous.length === next.length && previous.every((item, index) => {
    const other = next[index];
    return item.tool === other.tool && item.timestamp === other.timestamp;
  });
}
