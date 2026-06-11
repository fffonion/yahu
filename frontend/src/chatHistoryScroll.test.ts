import { describe, expect, test } from 'bun:test';
import { shouldLoadOlderFromWheel } from './chatHistoryScroll';

describe('chat history scroll triggers', () => {
  test('wheel-up at the top requests older history even when hidden tool messages leave no scroll delta', () => {
    expect(shouldLoadOlderFromWheel({ scrollTop: 0, scrollHeight: 420, clientHeight: 640 }, -24, true, false)).toBe(true);
  });

  test('does not request older history for wheel-down, missing older pages, or active load', () => {
    const atTop = { scrollTop: 0, scrollHeight: 420, clientHeight: 640 };
    expect(shouldLoadOlderFromWheel(atTop, 18, true, false)).toBe(false);
    expect(shouldLoadOlderFromWheel(atTop, -18, false, false)).toBe(false);
    expect(shouldLoadOlderFromWheel(atTop, -18, true, true)).toBe(false);
  });
});
