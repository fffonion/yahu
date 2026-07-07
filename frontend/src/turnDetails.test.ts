import { describe, expect, test } from 'bun:test';
import { buildDesktopTurnBlocks, buildTurnDetailItems } from './turnDetails';

type Msg = { id: string; role: string; content?: string; pending?: boolean; reasoning?: string; toolName?: string; toolCalls?: unknown };

const user: Msg = { id: 'u1', role: 'user', content: 'do it' };
const prelude: Msg = { id: 'a1', role: 'assistant', content: 'I will inspect', reasoning: 'plan', toolCalls: [{ id: 'call_1' }] };
const tool: Msg = { id: 't1', role: 'tool', content: '{"ok":true}', toolName: 'terminal' };
const final: Msg = { id: 'a2', role: 'assistant', content: 'final answer' };

describe('turn detail grouping', () => {
  test('groups tool and thinking messages between a user message and the final assistant answer', () => {
    const items = buildTurnDetailItems([user, prelude, tool, final]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message']);
    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1:a2' });
    if (items[1].kind !== 'detailGroup') throw new Error('expected detail group');
    expect(items[1].messages.map((message) => message.id)).toEqual(['a1', 't1']);
    expect(items[1].sourceIndexes).toEqual([1, 2]);
  });

  test('keeps streaming intermediate messages visible until the final assistant answer arrives', () => {
    const streamingFinal: Msg = { id: 'a2', role: 'assistant', content: 'partial answer', pending: true };
    const items = buildTurnDetailItems([user, prelude, tool, streamingFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message', 'message', 'message']);
  });

  test('does not fold normal multi-message history without a completed final answer', () => {
    const items = buildTurnDetailItems([user, prelude, tool]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message', 'message']);
  });

  test('desktop turn blocks wrap each user turn including detail groups and final answer', () => {
    const first = buildTurnDetailItems([user, prelude, tool, final]);
    const secondUser: Msg = { id: 'u2', role: 'user', content: 'next' };
    const pending: Msg = { id: 'a3', role: 'assistant', content: 'working', pending: true, reasoning: 'live' };
    const second = buildTurnDetailItems([secondUser, pending]);

    const blocks = buildDesktopTurnBlocks([...first, ...second]);

    expect(blocks).toHaveLength(2);
    expect(blocks[0].items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message']);
    expect(blocks[0].sourceIndexes).toEqual([0, 1, 2, 3]);
    expect(blocks[1].items.map((item) => item.kind)).toEqual(['message', 'message']);
    expect(blocks[1].sourceIndexes).toEqual([0, 1]);
  });
});
