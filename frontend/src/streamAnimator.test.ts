import { describe, expect, test } from 'bun:test';
import { createStreamAnimator, streamChunkSize } from './streamAnimator';

describe('streamChunkSize', () => {
  test('uses small chunks for a short backlog', () => {
    expect(streamChunkSize(1, false)).toBe(1);
    expect(streamChunkSize(20, false)).toBeLessThanOrEqual(2);
  });

  test('adapts upward for large backlog without dumping everything at once', () => {
    const streaming = streamChunkSize(900, false);
    const completing = streamChunkSize(900, true);
    expect(streaming).toBeGreaterThan(2);
    expect(streaming).toBeLessThan(900);
    expect(streaming).toBeLessThanOrEqual(24);
    expect(completing).toBeGreaterThan(streaming);
    expect(completing).toBeLessThanOrEqual(36);
  });
});

describe('createStreamAnimator', () => {
  test('buffers a burst and reveals it over multiple scheduled ticks', async () => {
    const scheduled: Array<() => void> = [];
    const updates: string[] = [];
    const animator = createStreamAnimator({
      onUpdate: (text) => updates.push(text),
      schedule: (cb) => {
        scheduled.push(cb);
        return scheduled.length;
      },
      cancel: () => {},
    });

    animator.append('abcdefghijklmnopqrstuvwxyz0123456789');

    expect(updates).toEqual([]);
    expect(scheduled.length).toBe(1);
    scheduled.shift()?.();
    expect(updates[0].length).toBeGreaterThan(0);
    expect(updates[0].length).toBeLessThan(36);
    expect(updates[0]).not.toBe('abcdefghijklmnopqrstuvwxyz0123456789');

    const done = animator.finish();
    while (scheduled.length) scheduled.shift()?.();
    await done;
    expect(updates.at(-1)).toBe('abcdefghijklmnopqrstuvwxyz0123456789');
  });
});
