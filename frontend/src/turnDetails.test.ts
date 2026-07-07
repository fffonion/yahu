import { describe, expect, test } from 'bun:test';
import { buildDesktopTurnBlocks, buildTurnDetailItems } from './turnDetails';

type Msg = { id: string; role: string; content?: string; pending?: boolean; reasoning?: string; toolName?: string; toolCalls?: unknown; turnDetails?: { count: number; toolCount?: number; thinkingCount?: number; afterId?: string; beforeId?: string } };

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

  test('creates a lazy detail group from skeleton metadata without preloaded detail messages', () => {
    const skeletonFinal: Msg = { id: 'a2', role: 'assistant', content: 'final answer', turnDetails: { count: 2, toolCount: 1, thinkingCount: 1, afterId: 'u1', beforeId: 'a2' } };
    const items = buildTurnDetailItems([user, skeletonFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message']);
    if (items[1].kind !== 'detailGroup') throw new Error('expected lazy detail group');
    expect(items[1].messages).toEqual([]);
    expect(items[1].detail).toEqual({ count: 2, toolCount: 1, thinkingCount: 1, afterId: 'u1', beforeId: 'a2' });
    expect(items[1].id).toBe('turn-details:u1:a2');
  });

  test('does not replace active streaming detail rows with a lazy skeleton group', () => {
    const streamingFinal: Msg = { id: 'a2', role: 'assistant', content: 'partial answer', pending: true, turnDetails: { count: 2, afterId: 'u1', beforeId: 'a2' } };
    const items = buildTurnDetailItems([user, prelude, tool, streamingFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message', 'message', 'message']);
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

  test('starts a new folded detail segment when tools appear after a completed final answer', () => {
    const nextPrelude: Msg = { id: 'a3', role: 'assistant', content: 'continuing from restored context', toolCalls: [{ id: 'call_2' }] };
    const nextTool: Msg = { id: 't2', role: 'tool', content: '{"ok":2}', toolName: 'terminal' };
    const nextFinal: Msg = { id: 'a4', role: 'assistant', content: 'second final' };
    const items = buildTurnDetailItems([user, prelude, tool, final, nextPrelude, nextTool, nextFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message', 'detailGroup', 'message']);
    expect(items[3]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:rootless:a4' });
    if (items[3].kind !== 'detailGroup') throw new Error('expected second detail group');
    expect(items[3].messages.map((message) => message.id)).toEqual(['a3', 't2']);
  });

  test('folds a trailing rootless tool segment at the loaded history window boundary', () => {
    const trailingTool: Msg = { id: 't2', role: 'tool', content: '{"ok":2}', toolName: 'terminal' };
    const items = buildTurnDetailItems([user, prelude, tool, final, trailingTool]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message', 'detailGroup']);
    expect(items[3]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:rootless:trailing-t2' });
    if (items[3].kind !== 'detailGroup') throw new Error('expected trailing detail group');
    expect(items[3].messages.map((message) => message.id)).toEqual(['t2']);
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
