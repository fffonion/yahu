import { describe, expect, test } from 'bun:test';
import { formatChatMessageTime, sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';

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

  test('keeps latest at least as new as session metadata when loaded messages come from older stitched history', () => {
    const times = sessionHeaderTimes(
      { id: 's1', started_at: 1783102276.8085577, last_active: 1783347141.9966545 },
      [
        { id: 'old-user', role: 'user', content: 'older stitched row', timestamp: 1718665260 },
        { id: 'old-assistant', role: 'assistant', content: 'older reply', timestamp: 1718665300 },
      ],
      stamp,
    );

    expect(times).toEqual({ started: 'Started T1783102276808', latest: 'Latest T1783347141996' });
  });

  test('parses ISO session timestamps from the API server for header labels', () => {
    const times = sessionHeaderTimes(
      { id: 's1', started_at: '2026-07-04T02:11:16.808Z', last_active: '2026-07-06T22:12:21.996Z' },
      [{ id: 'old', role: 'user', content: 'older loaded row', timestamp: '2026-06-18T03:01:00.000Z' }],
      stamp,
    );

    expect(times).toEqual({ started: 'Started T1783131076808', latest: 'Latest T1783375941996' });
  });

  test('formats today chat message timestamps as hour and minute only', () => {
    expect(formatChatMessageTime('2026-07-06T09:08:00', new Date('2026-07-06T22:00:00'))).toBe('09:08');
  });

  test('formats older chat message timestamps with month and day but no year', () => {
    expect(formatChatMessageTime('2026-07-05T09:08:00', new Date('2026-07-06T22:00:00'))).toBe('07/05 09:08');
  });
});
