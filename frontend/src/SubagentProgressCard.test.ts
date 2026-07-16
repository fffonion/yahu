import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { GoalReason } from './SubagentProgressCard';
import type { PersistentGoal } from './subagentProgress';

function goal(overrides: Partial<PersistentGoal>): PersistentGoal {
  return {
    text: 'Improve the service',
    status: 'active',
    turnsUsed: 1,
    maxTurns: 20,
    subgoals: [],
    todos: [],
    ...overrides,
  };
}

describe('GoalReason', () => {
  test('renders pausedReason ahead of lastReason for a paused goal', () => {
    const pausedGoal = goal({
      status: 'paused',
      pausedReason: 'Waiting for approval',
      lastReason: 'Older checkpoint note',
    });
    const html = renderToStaticMarkup(React.createElement(GoalReason, { goal: pausedGoal }));

    expect(html).toContain('Waiting for approval');
    expect(html).not.toContain('Older checkpoint note');
  });

  test('falls back to lastReason when a paused goal has no pausedReason', () => {
    const pausedGoal = goal({
      status: 'paused',
      lastReason: 'Paused after validation',
    });
    const html = renderToStaticMarkup(React.createElement(GoalReason, { goal: pausedGoal }));

    expect(html).toContain('Paused after validation');
  });
});