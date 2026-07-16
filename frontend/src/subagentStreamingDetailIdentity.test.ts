import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mergeSubagentMessages, normalizeSubagentMessages } from './subagentProgress';
import { buildTurnDetailItems } from './turnDetails';

const snapshot = (tail: unknown[] = []) => ({
  data: [
    { id: 'user-1', role: 'user', content: 'Investigate streaming detail' },
    { id: 'assistant-1', role: 'assistant', content: '', tool_calls: [{ id: 'call-1', function: { name: 'read_file', arguments: '{}' } }] },
    { id: 'tool-1', role: 'tool', tool_name: 'read_file', tool_call_id: 'call-1', content: 'first result' },
    ...tail,
  ],
});

describe('streaming subagent detail identity', () => {
  test('wires detail refreshes through the incremental merge', () => {
    const card = readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
    expect(card).toContain('setMessages((current) => mergeSubagentMessages(current, items));');
    expect(card).not.toContain('setMessages(items);');
  });

  test('incrementally reuses unchanged messages and the complete array when nothing changed', () => {
    const previous = normalizeSubagentMessages(snapshot());
    const unchanged = mergeSubagentMessages(previous, normalizeSubagentMessages(snapshot()));
    expect(unchanged).toBe(previous);

    const appended = mergeSubagentMessages(previous, normalizeSubagentMessages(snapshot([
      { id: 'tool-2', role: 'tool', tool_name: 'terminal', content: 'second result' },
    ])));
    expect(appended).not.toBe(previous);
    expect(appended.slice(0, 3).every((message, index) => message === previous[index])).toBe(true);
    expect(appended[3].id).toBe('tool-2');

    const changed = mergeSubagentMessages(appended, normalizeSubagentMessages({ data: [
      ...snapshot().data.slice(0, 2),
      { id: 'tool-1', role: 'tool', tool_name: 'read_file', tool_call_id: 'call-1', content: 'updated result' },
      { id: 'tool-2', role: 'tool', tool_name: 'terminal', content: 'second result' },
    ] }));
    expect(changed[0]).toBe(appended[0]);
    expect(changed[1]).toBe(appended[1]);
    expect(changed[2]).not.toBe(appended[2]);
    expect(changed[3]).toBe(appended[3]);
  });

  test('keeps the unfinished turn detail key stable as streaming messages append', () => {
    const first = buildTurnDetailItems(normalizeSubagentMessages(snapshot()));
    const next = buildTurnDetailItems(normalizeSubagentMessages(snapshot([
      { id: 'tool-2', role: 'tool', tool_name: 'terminal', content: 'second result' },
    ])));
    const firstGroup = first.find((item) => item.kind === 'detailGroup');
    const nextGroup = next.find((item) => item.kind === 'detailGroup');
    expect(firstGroup?.id).toBe('turn-details:user-1');
    expect(nextGroup?.id).toBe(firstGroup?.id);
  });
});
