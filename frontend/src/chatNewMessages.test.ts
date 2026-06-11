import { describe, expect, test } from 'bun:test';
import { computeNewMessageMarker, findNewMessageSplitIndex } from './chatNewMessages';

describe('new message marker helpers', () => {
  test('marks the first newly visible message while excluding pending placeholders from the bubble count', () => {
    const previous = [{ id: 'm1', role: 'assistant', content: 'old' }];
    const next = [
      ...previous,
      { id: 'm2', role: 'user', content: 'remote user' },
      { id: 'other-platform-pending', role: 'assistant', content: '', pending: true },
    ];

    const marker = computeNewMessageMarker(previous, next, '');

    expect(marker).toEqual({ firstId: 'm2', count: 1 });
    expect(findNewMessageSplitIndex(next, marker.firstId)).toBe(1);
  });

  test('keeps the divider before the first unseen message as a pending placeholder resolves', () => {
    const previous = [
      { id: 'm1', role: 'assistant', content: 'old' },
      { id: 'm2', role: 'user', content: 'remote user' },
      { id: 'other-platform-pending', role: 'assistant', content: '', pending: true },
    ];
    const next = [
      { id: 'm1', role: 'assistant', content: 'old' },
      { id: 'm2', role: 'user', content: 'remote user' },
      { id: 'm3', role: 'assistant', content: 'remote assistant' },
    ];

    const marker = computeNewMessageMarker(previous, next, 'm2');

    expect(marker).toEqual({ firstId: 'm2', count: 2 });
    expect(findNewMessageSplitIndex(next, marker.firstId)).toBe(1);
  });

  test('does not create a marker for watch updates that only modify existing messages', () => {
    const previous = [{ id: 'm1', role: 'assistant', content: 'hel' }];
    const next = [{ id: 'm1', role: 'assistant', content: 'hello' }];

    expect(computeNewMessageMarker(previous, next, '')).toEqual({ firstId: '', count: 0 });
  });
});
