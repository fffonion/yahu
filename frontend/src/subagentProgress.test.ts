import { describe, expect, test } from 'bun:test';
import {
  buildSubagentTree,
  formatSubagentFinalMessages,
  latestSubagent,
  isSubagentDetailNearBottom,
  normalizeSubagentMessages,
  normalizeSubagentSnapshot,
  parseSubagentFinalStructuredContent,
  previewSubagent,
  subagentIteration,
  subagentMessagesUrl,
  subagentWebSocketUrl,
} from './subagentProgress';

describe('subagent progress websocket projection', () => {
  test('builds a same-origin websocket URL and percent-encodes the session id', () => {
    expect(subagentWebSocketUrl({ protocol: 'https:', host: 'yahu.example' }, 'session/with space')).toBe(
      'wss://yahu.example/chat/subagents/session%2Fwith%20space/ws',
    );
    expect(subagentWebSocketUrl({ protocol: 'http:', host: '127.0.0.1:9642' }, 's1')).toBe(
      'ws://127.0.0.1:9642/chat/subagents/s1/ws',
    );
  });

  test('normalizes websocket snapshots and rejects another session', () => {
    expect(normalizeSubagentSnapshot({ type: 'subagents.snapshot', session_id: 'other', subagents: [] }, 's1')).toBeNull();
    expect(normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 's1',
      generated_at: 100,
      subagents: [{
        session_id: 'child',
        parent_session_id: 's1',
        task: 'Review code',
        status: 'running',
        started_at: 90,
        message_count: 4,
        tool_count: 2,
        api_calls: 1,
        current_tool: 'terminal',
        todos: [{ id: 'test', content: 'Run tests', status: 'in_progress' }],
        activity: [{ tool: 'read_file', timestamp: 95 }],
      }],
    }, 's1')).toMatchObject({
      sessionId: 's1',
      subagents: [{ sessionId: 'child', task: 'Review code', status: 'running', currentTool: 'terminal' }],
    });
  });

  test('normalizes complete lazy-loaded conversation details without truncating text', () => {
    expect(subagentMessagesUrl('session/with space')).toBe('/chat/subagents/session%2Fwith%20space/messages');
    expect(normalizeSubagentMessages({ data: [
      { id: 1, role: 'assistant', content: 'Final **answer**', reasoning: 'Long reasoning text', timestamp: 10, tool_calls: [{ id: 'call-1', function: { name: 'read_file', arguments: '{"path":"src/App.tsx"}' } }] },
      { id: 2, role: 'tool', tool_name: 'read_file', tool_call_id: 'call-1', content: 'full\noutput', timestamp: 11 },
    ] })).toMatchObject([
      { id: '1', role: 'assistant', content: 'Final **answer**', reasoning: 'Long reasoning text', timestamp: 10, toolCalls: [{ id: 'call-1' }] },
      { id: '2', role: 'tool', toolName: 'read_file', toolCallId: 'call-1', content: 'full\noutput', timestamp: 11 },
    ]);
  });

  test('marks only a valid JSON final assistant message for shared structured rendering', () => {
    const messages = normalizeSubagentMessages({ data: [
      { id: 1, role: 'user', content: '{"leave":"user json alone"}' },
      { id: 2, role: 'assistant', content: 'Working text' },
      { id: 3, role: 'tool', content: '{"leave":"tool json alone"}', tool_name: 'read_file' },
      { id: 4, role: 'assistant', content: '{"passed":true,"items":[1,2]}' },
    ] });

    const formatted = formatSubagentFinalMessages(messages);
    expect(formatted.map((message) => message.content)).toEqual(messages.map((message) => message.content));
    expect(formatted.slice(0, 3).every((message) => message.structuredContent === undefined)).toBe(true);
    expect(formatted[3].structuredContent).toEqual({ value: { passed: true, items: [1, 2] } });
    expect(messages[3].structuredContent).toBeUndefined();
  });

  test('keeps non-JSON final text unchanged and parses JSON summary fallbacks', () => {
    expect(parseSubagentFinalStructuredContent('Normal final answer')).toBeUndefined();
    expect(parseSubagentFinalStructuredContent('{"summary":"ok","errors":[]}')).toEqual({ value: { summary: 'ok', errors: [] } });
    expect(parseSubagentFinalStructuredContent('null')).toEqual({ value: null });
  });

  test('builds a nested tree from parent session ids', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      subagents: [
        { session_id: 'root', parent_session_id: 'parent', task: 'Root', status: 'completed' },
        { session_id: 'leaf', parent_session_id: 'root', task: 'Leaf', status: 'running' },
      ],
    }, 'parent')!;

    const tree = buildSubagentTree(snapshot.subagents, 'parent');

    expect(tree).toHaveLength(1);
    expect(tree[0].sessionId).toBe('root');
    expect(tree[0].children.map((item) => item.sessionId)).toEqual(['leaf']);
  });

  test('orders recent root and nested subagent lists newest first', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      subagents: [
        { session_id: 'old', parent_session_id: 'parent', task: 'Old', status: 'completed', started_at: 10 },
        { session_id: 'new', parent_session_id: 'parent', task: 'New', status: 'completed', started_at: 30 },
        { session_id: 'middle', parent_session_id: 'parent', task: 'Middle', status: 'completed', started_at: 20 },
        { session_id: 'child-old', parent_session_id: 'new', task: 'Child old', status: 'completed', started_at: 31 },
        { session_id: 'child-new', parent_session_id: 'new', task: 'Child new', status: 'completed', started_at: 32 },
      ],
    }, 'parent')!;

    const tree = buildSubagentTree(snapshot.subagents, 'parent');

    expect(tree.map((item) => item.sessionId)).toEqual(['new', 'middle', 'old']);
    expect(tree[0].children.map((item) => item.sessionId)).toEqual(['child-new', 'child-old']);
  });

  test('selects the latest subagent for the collapsed panel preview', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      subagents: [
        { session_id: 'older', parent_session_id: 'parent', task: 'Older', status: 'completed', started_at: 100 },
        { session_id: 'newest', parent_session_id: 'parent', task: 'Newest', status: 'running', started_at: 300 },
        { session_id: 'middle', parent_session_id: 'parent', task: 'Middle', status: 'completed', started_at: 200 },
      ],
    }, 'parent')!;

    expect(latestSubagent(snapshot.subagents)?.sessionId).toBe('newest');
    expect(latestSubagent([])).toBeUndefined();
  });

  test('reports the current agent iteration from persisted API call progress', () => {
    expect(subagentIteration({ status: 'running', apiCalls: 0 })).toBe(1);
    expect(subagentIteration({ status: 'running', apiCalls: 4 })).toBe(4);
    expect(subagentIteration({ status: 'completed', apiCalls: 4 })).toBe(4);
    expect(subagentIteration({ status: 'completed', apiCalls: 0 })).toBe(0);
  });

  test('keeps the persistent goal distinct from a running subagent task', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      goal: {
        text: 'Optimize the interpreter',
        status: 'active',
        turns_used: 4,
        max_turns: 20,
        subgoals: [],
        todos: [{ id: 'main-test', content: 'Run the main-session tests', status: 'in_progress' }],
      },
      subagents: [
        { session_id: 'child', parent_session_id: 'parent', task: 'Profile the hot path', status: 'running' },
      ],
    }, 'parent')!;

    expect(snapshot.goal).toEqual({
      text: 'Optimize the interpreter',
      status: 'active',
      turnsUsed: 4,
      maxTurns: 20,
      subgoals: [],
      todos: [{ id: 'main-test', content: 'Run the main-session tests', status: 'in_progress' }],
    });
    expect(snapshot.subagents[0].task).toBe('Profile the hot path');
  });

  test('prefers the latest running subagent even when a newer subagent already completed', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      subagents: [
        { session_id: 'running-only', parent_session_id: 'parent', task: 'Active task', status: 'running', started_at: 100 },
        { session_id: 'completed-new', parent_session_id: 'parent', task: 'Finished task', status: 'completed', started_at: 300 },
      ],
    }, 'parent')!;

    expect(latestSubagent(snapshot.subagents)?.sessionId).toBe('completed-new');
    expect(previewSubagent(snapshot.subagents)?.sessionId).toBe('running-only');
  });

  test('detects when a long subagent detail remains close enough to its latest content', () => {
    expect(isSubagentDetailNearBottom({ scrollTop: 900, scrollHeight: 1500, clientHeight: 520 })).toBe(true);
    expect(isSubagentDetailNearBottom({ scrollTop: 500, scrollHeight: 1500, clientHeight: 520 })).toBe(false);
    expect(isSubagentDetailNearBottom({ scrollTop: 0, scrollHeight: 400, clientHeight: 520 })).toBe(true);
  });
});
