import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { GoalMilestones, SubagentProgressNode } from './SubagentProgressCard';
import type { PersistentGoal, SubagentTreeNode } from './subagentProgress';

function goal(overrides: Partial<PersistentGoal>): PersistentGoal {
  return {
    text: 'Improve the service',
    status: 'active',
    turnsUsed: 1,
    maxTurns: 20,
    subgoals: [],
    todos: [],
    milestones: [],
    ...overrides,
  };
}

describe('GoalMilestones', () => {
  test('renders every round newest first', () => {
    const activeGoal = goal({
      milestones: [
        { turn: 1, timestamp: 100, verdict: 'continue', reason: 'First checkpoint' },
        { turn: 3, timestamp: 300, verdict: 'continue', reason: 'Latest checkpoint' },
        { turn: 2, timestamp: 200, verdict: 'continue', reason: 'Middle checkpoint' },
      ],
    });
    const html = renderToStaticMarkup(React.createElement(GoalMilestones, { goal: activeGoal }));

    expect(html).toContain('Milestones');
    expect(html.indexOf('Latest checkpoint')).toBeLessThan(html.indexOf('Middle checkpoint'));
    expect(html.indexOf('Middle checkpoint')).toBeLessThan(html.indexOf('First checkpoint'));
  });

  test('adds a distinct pause milestone without hiding earlier rounds', () => {
    const pausedGoal = goal({
      status: 'paused',
      pausedReason: 'Waiting for approval',
      milestones: [{ turn: 1, timestamp: 100, verdict: 'continue', reason: 'Older checkpoint' }],
    });
    const html = renderToStaticMarkup(React.createElement(GoalMilestones, { goal: pausedGoal }));

    expect(html).toContain('Waiting for approval');
    expect(html).toContain('Older checkpoint');
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
      detailCache: {},
      onMessagesLoaded: () => {},
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