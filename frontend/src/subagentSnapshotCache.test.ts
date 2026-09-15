import { beforeEach, describe, expect, test } from 'bun:test';
import {
  clearSubagentSnapshotCache,
  readCachedSubagentSnapshot,
  sameSubagentSnapshot,
  SUBAGENT_SNAPSHOT_CACHE_TTL_MS,
  writeCachedSubagentSnapshot,
} from './subagentSnapshotCache';
import type { SubagentProgressSnapshot } from './subagentProgress';

function snapshot(overrides: Partial<SubagentProgressSnapshot> = {}): SubagentProgressSnapshot {
  return {
    sessionId: 'session-1',
    generatedAt: 100,
    subagents: [],
    ...overrides,
  };
}

describe('subagent browser snapshot cache', () => {
  beforeEach(() => clearSubagentSnapshotCache());

  test('keeps a session snapshot for five minutes and expires it at the boundary', () => {
    const value = snapshot();
    writeCachedSubagentSnapshot(value, 1_000);

    expect(SUBAGENT_SNAPSHOT_CACHE_TTL_MS).toBe(5 * 60 * 1000);
    expect(readCachedSubagentSnapshot('session-1', 1_000 + SUBAGENT_SNAPSHOT_CACHE_TTL_MS - 1)).toBe(value);
    expect(readCachedSubagentSnapshot('session-1', 1_000 + SUBAGENT_SNAPSHOT_CACHE_TTL_MS)).toBeNull();
  });

  test('ignores generated time when deciding whether the status bar changed', () => {
    const previous = snapshot({ generatedAt: 100 });
    const next = snapshot({ generatedAt: 200 });

    expect(sameSubagentSnapshot(previous, next)).toBe(true);
    expect(sameSubagentSnapshot(previous, snapshot({ error: 'temporary failure' }))).toBe(false);
  });

  test('detects changed goal and subagent content', () => {
    const previous = snapshot({
      goal: {
        text: 'Ship the change',
        status: 'active',
        turnsUsed: 1,
        maxTurns: 5,
        subgoals: [],
        todos: [],
        milestones: [],
      },
    });
    const next = snapshot({
      goal: {
        ...previous.goal!,
        turnsUsed: 2,
      },
      subagents: [{
        sessionId: 'child-1',
        parentSessionId: 'session-1',
        ancestryOmitted: false,
        task: 'Inspect the UI',
        status: 'running',
        messageCount: 1,
        toolCount: 0,
        apiCalls: 0,
        todos: [],
        activity: [],
      }],
    });

    expect(sameSubagentSnapshot(previous, next)).toBe(false);
  });
});
