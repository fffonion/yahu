import React from 'react';
import { beforeEach, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { GoalMilestones, SubagentGoalPanel, SubagentProgressNode } from './SubagentProgressCard';
import { setLang } from './i18n';
import type { PersistentGoal, SubagentTreeNode } from './subagentProgress';

beforeEach(() => setLang('en'));

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

describe('completed Goal panel', () => {
  test('keeps the full goal, frozen elapsed time and recorded steps', () => {
    const done = goal({
      status: 'done', createdAt: 100, lastTurnAt: 5500, turnsUsed: 8,
      subgoals: ['Preserve the verified result'],
      todos: [{ id: 'verify', content: 'Verify the final build', status: 'completed' }],
      milestones: [{ turn: 8, timestamp: 5500, verdict: 'done', reason: 'All checks passed' }],
    });
    const render = (nowSeconds: number) => renderToStaticMarkup(React.createElement(SubagentGoalPanel, { goal: done, nowSeconds }));
    const html = render(6000);
    expect(html).toContain('data-status="done"');
    expect(html).toContain('Done');
    expect(html).toContain('1 hr 30 min');
    expect(html).toContain('8/20 turns');
    expect(html).toContain('Preserve the verified result');
    expect(html).toContain('Verify the final build');
    expect(html).toContain('All checks passed');
    expect(render(90000)).toBe(html);
  });

  test('applies the same completion green to Goal content and completed todos', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    for (const selector of [
      '.subagent-goal-panel .subagent-progress-todos li.completed',
      '.subagent-goal-panel[data-status="done"] .subagent-goal-body',
      '.subagent-goal-panel[data-status="done"] .subagent-goal-milestones li p',
      '.subagent-goal-panel[data-status="done"] .subagent-goal-meta',
      '.subagent-goal-panel[data-status="done"] .subagent-goal-footer',
    ]) expect(css.includes(`${selector}{color:var(--green)}`)).toBe(true);
  });

  test('uses the theme green accent only for completed goals', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css.includes('.subagent-goal-panel[data-status="done"]{--subagent-goal-accent:var(--green)}')).toBe(true);
    const active = renderToStaticMarkup(React.createElement(SubagentGoalPanel, { goal: goal({}), nowSeconds: 100 }));
    expect(active).not.toContain('data-status="done"');
  });
});

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
  test('shows a stop control only for a running child detail', () => {
    const runningNode: SubagentTreeNode = {
      sessionId: 'running-child',
      parentSessionId: 'parent',
      ancestryOmitted: false,
      task: 'Long task',
      status: 'running',
      startedAt: 10,
      messageCount: 0,
      toolCount: 0,
      apiCalls: 1,
      todos: [],
      activity: [],
      children: [],
    };
    const renderNode = (node: SubagentTreeNode) => renderToStaticMarkup(React.createElement(SubagentProgressNode, {
      node,
      openNodeIds: new Set([node.sessionId]),
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

    expect(renderNode(runningNode)).toContain('class="subagent-progress-stop"');
    expect(renderNode(runningNode)).toContain('Stop');
    expect(renderNode({ ...runningNode, status: 'completed', endedAt: 20 })).not.toContain('subagent-progress-stop');
  });

  test('keeps the stop action inset from the detail edge', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.subagent-progress-actions{display:flex;justify-content:flex-end;padding-right:8px');
  });

  test('omits any visible parent-omitted marker for completed children', () => {
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

    expect(html).toContain('Finished child</strong>');
    expect(html).not.toContain('parent omitted');
    expect(html).not.toContain('父级未显示');
    expect(html).not.toContain('subagent-progress-omitted-ancestry');
  });
});