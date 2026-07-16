import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { GoalReason, SubagentProgressNode } from './SubagentProgressCard';
import type { PersistentGoal, SubagentTreeNode } from './subagentProgress';

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

describe('SubagentProgressNode', () => {
  test('keeps a completed omitted-parent marker neutral and inside the compact description line', () => {
    const node: SubagentTreeNode = {
      sessionId: 'child',
      parentSessionId: 'hidden-parent',
      ancestryOmitted: true,
      task: 'Finished child',
      status: 'completed',
      startedAt: 10,
      endedAt: 20,
      messageCount: 2,
      toolCount: 0,
      apiCalls: 1,
      todos: [],
      activity: [],
      children: [],
    };
    const html = renderToStaticMarkup(React.createElement(SubagentProgressNode, {
      node,
      openNodeIds: new Set<string>(),
      onOpenChange: () => {},
      nowSeconds: 30,
      depth: 0,
      showReasoning: true,
      showToolCalls: true,
      compact: false,
      onDetailOpen: () => {},
      onDetailContentChange: () => {},
    }));

    expect(html).toContain('Finished child<span class="subagent-progress-omitted-ancestry" title="parent omitted"> · parent omitted</span></strong>');
    expect(html).not.toContain('outside window');
    expect(html).not.toContain('<small class="subagent-progress-omitted-ancestry"');
  });
});