import { describe, expect, test } from 'bun:test';
import { currentNavigatorId } from './chatUserNavigator';

describe('chat user navigator selection', () => {
  test('returns one current line when several user messages share the visible range', () => {
    const items = [{ id: 'u1' }, { id: 'u2' }, { id: 'u3' }, { id: 'u4' }];

    expect(currentNavigatorId(items, new Set(['u2', 'u3']))).toBe('u3');
    expect(currentNavigatorId(items, new Set())).toBeUndefined();
  });
});
