import { describe, expect, test } from 'bun:test';
import { chatLatestButtonVisible } from './chatLatest';

describe('chat latest navigation', () => {
  test('shows the control only when newer history exists or the viewport left the latest threshold', () => {
    const bottom = { scrollHeight: 1000, scrollTop: 500, clientHeight: 500 };
    expect(chatLatestButtonVisible(bottom, false)).toBe(false);
    expect(chatLatestButtonVisible({ ...bottom, scrollTop: 381 }, false)).toBe(false);
    expect(chatLatestButtonVisible({ ...bottom, scrollTop: 379 }, false)).toBe(true);
    expect(chatLatestButtonVisible(bottom, true)).toBe(true);
  });
});
