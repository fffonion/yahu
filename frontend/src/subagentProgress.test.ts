import { describe, expect, test } from 'bun:test';
import {
  buildSubagentTree,
  normalizeSubagentMessages,
  normalizeSubagentSnapshot,
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
        goal: 'Review code',
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
      subagents: [{ sessionId: 'child', goal: 'Review code', status: 'running', currentTool: 'terminal' }],
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

  test('builds a nested tree from parent session ids', () => {
    const snapshot = normalizeSubagentSnapshot({
      type: 'subagents.snapshot',
      session_id: 'parent',
      subagents: [
        { session_id: 'root', parent_session_id: 'parent', goal: 'Root', status: 'completed' },
        { session_id: 'leaf', parent_session_id: 'root', goal: 'Leaf', status: 'running' },
      ],
    }, 'parent')!;

    const tree = buildSubagentTree(snapshot.subagents, 'parent');

    expect(tree).toHaveLength(1);
    expect(tree[0].sessionId).toBe('root');
    expect(tree[0].children[0].sessionId).toBe('leaf');
  });
});
