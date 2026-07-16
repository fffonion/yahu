import { describe, expect, test } from 'bun:test';
import { buildDesktopTurnBlocks, buildTurnDetailItems } from './turnDetails';

type Msg = { id: string; role: string; content?: string; pending?: boolean; reasoning?: string; toolName?: string; toolCalls?: unknown; historyGap?: { after: number; before: number }; turnDetails?: { count: number; toolCount?: number; thinkingCount?: number; afterId?: string; beforeId?: string } };

const user: Msg = { id: 'u1', role: 'user', content: 'do it' };
const prelude: Msg = { id: 'a1', role: 'assistant', content: 'I will inspect', reasoning: 'plan', toolCalls: [{ id: 'call_1' }] };
const tool: Msg = { id: 't1', role: 'tool', content: '{"ok":true}', toolName: 'terminal' };
const final: Msg = { id: 'a2', role: 'assistant', content: 'final answer' };

describe('turn detail grouping', () => {
  test('groups tool and thinking messages between a user message and the final assistant answer', () => {
    const items = buildTurnDetailItems([user, prelude, tool, final]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message']);
    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1' });
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
    expect(items[1].id).toBe('turn-details:u1');
  });

  test('keeps active streaming detail rows inside the expanded turn frame', () => {
    const streamingFinal: Msg = { id: 'a2', role: 'assistant', content: 'partial answer', pending: true, turnDetails: { count: 2, afterId: 'u1', beforeId: 'a2' } };
    const items = buildTurnDetailItems([user, prelude, tool, streamingFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup']);
    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1', defaultOpen: true });
    if (items[1].kind !== 'detailGroup') throw new Error('expected streaming detail group');
    expect(items[1].messages.map((message) => message.id)).toEqual(['a1', 't1', 'a2']);
  });

  test('keeps streaming intermediate messages visible in an open frame until the final assistant answer arrives', () => {
    const streamingFinal: Msg = { id: 'a2', role: 'assistant', content: 'partial answer', pending: true };
    const items = buildTurnDetailItems([user, prelude, tool, streamingFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup']);
    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1', defaultOpen: true });
  });

  test('defaults unfinished user-turn details open when no final assistant answer exists', () => {
    const items = buildTurnDetailItems([user, prelude, tool]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup']);
    if (items[1].kind !== 'detailGroup') throw new Error('expected unfinished detail group');
    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1', defaultOpen: true });
    expect(items[1].messages.map((message) => message.id)).toEqual(['a1', 't1']);
  });

  test('completed final-answer detail groups stay closed by default', () => {
    const items = buildTurnDetailItems([user, prelude, tool, final]);

    expect(items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1' });
    if (items[1].kind !== 'detailGroup') throw new Error('expected completed detail group');
    expect(items[1].defaultOpen).toBeUndefined();
  });

  test('keeps the same detail-group identity when a streaming turn completes', () => {
    const streamingFinal: Msg = { id: 'a2', role: 'assistant', content: 'partial answer', pending: true };
    const streamingItems = buildTurnDetailItems([user, prelude, tool, streamingFinal]);
    const completedItems = buildTurnDetailItems([user, prelude, tool, final]);

    expect(streamingItems[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1', defaultOpen: true });
    expect(completedItems[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u1' });
  });

  test('uses a history coverage gap as a hard turn boundary', () => {
    const gap: Msg = { id: 'gap1', role: 'system', content: 'History coverage gap', historyGap: { after: 5, before: 90_000 } };
    const laterAssistant: Msg = { id: 'a-later', role: 'assistant', content: 'retained later response' };
    const items = buildTurnDetailItems([user, gap, laterAssistant]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message', 'message']);
    expect(items[1]).toMatchObject({ kind: 'message', message: gap });
    const blocks = buildDesktopTurnBlocks(items);
    expect(blocks).toHaveLength(3);
  });

  test('keeps preserved task state as a standalone item outside turn details', () => {
    const state: Msg = {
      id: 'state1',
      role: 'user',
      content: '[Your active task list was preserved across context compression]\n- [>] verify. Build and deploy (in_progress)\n- [ ] ship. Commit and push (pending)',
    };
    const items = buildTurnDetailItems([user, prelude, tool, state, final]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message', 'message', 'sessionState', 'message']);
    expect(items[3]).toMatchObject({ kind: 'sessionState', id: 'session-state:state1', message: state });
    const blocks = buildDesktopTurnBlocks(items);
    expect(blocks[1].items.map((item) => item.kind)).toEqual(['sessionState', 'message']);
  });

  test('keeps streaming details expanded in the same framed turn after session state', () => {
    const state: Msg = {
      id: 'state2',
      role: 'user',
      content: '[Your active task list was preserved across context compression]\n- [>] verify. Build and deploy (in_progress)',
    };
    const blocks = buildDesktopTurnBlocks(buildTurnDetailItems([state, prelude, tool]));

    expect(blocks).toHaveLength(1);
    expect(blocks[0].items.map((item) => item.kind)).toEqual(['sessionState', 'detailGroup']);
    const details = blocks[0].items[1];
    expect(details).toMatchObject({ kind: 'detailGroup', defaultOpen: true });
  });

  test('keeps bracketed tool output and assistant pre-tool context inside one detail group', () => {
    const skippedTool: Msg = {
      id: 't-skipped',
      role: 'tool',
      content: '[Tool execution skipped — terminal was not started. User sent a new message]',
      toolName: 'terminal',
    };
    const items = buildTurnDetailItems([user, prelude, skippedTool, tool, final]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message']);
    if (items[1].kind !== 'detailGroup') throw new Error('expected one detail group');
    expect(items[1].messages.map((message) => message.id)).toEqual(['a1', 't-skipped', 't1']);
    expect(items[1].messages[0].content).toBe('I will inspect');
  });

  test('places a standalone session state before the following user turn without affecting its frame', () => {
    const state: Msg = {
      id: 'state3',
      role: 'user',
      content: '[Your active task list was preserved across context compression]\n- [>] verify. Build and deploy (in_progress)',
    };
    const blocks = buildDesktopTurnBlocks(buildTurnDetailItems([state, user, prelude, tool]));

    expect(blocks).toHaveLength(2);
    expect(blocks[0].items.map((item) => item.kind)).toEqual(['sessionState']);
    expect(blocks[1].items.map((item) => item.kind)).toEqual(['message', 'detailGroup']);
    expect(blocks[1].items[1]).toMatchObject({ kind: 'detailGroup', defaultOpen: true });
  });

  test('renders Hermes prior-context and compaction summaries as a special block, not as a final answer', () => {
    const priorContext: Msg = {
      id: 'ctx1',
      role: 'assistant',
      content: `[PRIOR CONTEXT -- for reference only; not a new message]\nold transcript\n\n[END OF PRIOR CONTEXT -- COMPACTION SUMMARY BELOW]\n\n[CONTEXT COMPACTION -- REFERENCE ONLY]\nsummary\n--- END OF CONTEXT SUMMARY -- respond to the message below, not the summary above ---`,
    };
    const items = buildTurnDetailItems([user, priorContext, final]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'specialContextGroup', 'message']);
    expect(items[1]).toMatchObject({ kind: 'specialContextGroup', id: 'special-context:u1:ctx1' });
    if (items[1].kind !== 'specialContextGroup') throw new Error('expected special context group');
    expect(items[1].messages.map((message) => message.id)).toEqual(['ctx1']);
    expect(items[2]).toMatchObject({ kind: 'message', message: final });
  });

  test('keeps a trailing Hermes compaction summary in a special block without waiting for a later final answer', () => {
    const compaction: Msg = { id: 'ctx2', role: 'assistant', content: '[CONTEXT COMPACTION -- REFERENCE ONLY]\nsummary only' };
    const items = buildTurnDetailItems([user, compaction]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'specialContextGroup']);
    if (items[1].kind !== 'specialContextGroup') throw new Error('expected special context group');
    expect(items[1].messages.map((message) => message.id)).toEqual(['ctx2']);
  });

  test('does not treat normal assistant replies that quote context markers as special context', () => {
    const quotedMarkers: Msg = {
      id: 'a-quoted',
      role: 'assistant',
      content: '搞定。部署的对话页确认：\n\n- 页面中已有 1 个 `.special-context-block`\n\n这就是你说的那种 `[PRIOR CONTEXT -- for reference only]` / `[CONTEXT COMPACTION -- REFERENCE ONLY]` 消息的处理。',
    };
    const items = buildTurnDetailItems([user, quotedMarkers]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'message']);
    expect(items[1]).toMatchObject({ kind: 'message', message: quotedMarkers });
  });

  test('starts a new folded detail segment when tools appear after a completed final answer', () => {
    const nextPrelude: Msg = { id: 'a3', role: 'assistant', content: 'continuing from restored context', toolCalls: [{ id: 'call_2' }] };
    const nextTool: Msg = { id: 't2', role: 'tool', content: '{"ok":2}', toolName: 'terminal' };
    const nextFinal: Msg = { id: 'a4', role: 'assistant', content: 'second final' };
    const items = buildTurnDetailItems([user, prelude, tool, final, nextPrelude, nextTool, nextFinal]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message', 'detailGroup', 'message']);
    expect(items[3]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:rootless:a3' });
    if (items[3].kind !== 'detailGroup') throw new Error('expected second detail group');
    expect(items[3].messages.map((message) => message.id)).toEqual(['a3', 't2']);
  });

  test('folds a trailing rootless tool segment at the loaded history window boundary', () => {
    const trailingTool: Msg = { id: 't2', role: 'tool', content: '{"ok":2}', toolName: 'terminal' };
    const items = buildTurnDetailItems([user, prelude, tool, final, trailingTool]);

    expect(items.map((item) => item.kind)).toEqual(['message', 'detailGroup', 'message', 'detailGroup']);
    expect(items[3]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:rootless:t2' });
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
    expect(blocks[1].items.map((item) => item.kind)).toEqual(['message', 'detailGroup']);
    expect(blocks[1].items[1]).toMatchObject({ kind: 'detailGroup', id: 'turn-details:u2', defaultOpen: true });
    expect(blocks[1].sourceIndexes).toEqual([0, 1]);
  });
});
