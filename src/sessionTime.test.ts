import { describe, expect, test } from 'bun:test';
import { sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';

const stamp = (date: Date) => `T${date.getTime()}`;

describe('session time display helpers', () => {
  test('uses a formatted start time as the frontend-only title for untitled sessions', () => {
    expect(sessionDisplayTitle({ id: 's1', title: '', preview: 'do not use preview', started_at: 1700000000 }, stamp)).toBe('T1700000000000');
  });

  test('keeps real session titles when present', () => {
    expect(sessionDisplayTitle({ id: 's1', title: '  Real title  ', started_at: 1700000000 }, stamp)).toBe('Real title');
  });

  test('builds start and latest message labels for the opened session header', () => {
    const times = sessionHeaderTimes(
      { id: 's1', started_at: 1700000000, last_active: 1700000060 },
      [
        { id: 'm1', role: 'user', content: 'older', timestamp: 1700000030 },
        { id: 'm2', role: 'assistant', content: 'newer', timestamp: 1700000090000 },
      ],
      stamp,
    );

    expect(times).toEqual({ started: 'Started T1700000000000', latest: 'Latest T1700000090000' });
  });
});
