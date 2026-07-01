import { describe, expect, test } from 'bun:test';
import { splitSidebarSessions } from './sessionListFilter';

describe('session list cron filter', () => {
  const sessions = [
    { id: 'normal-1', source: 'telegram' },
    { id: 'cron-1', source: 'cron' },
    { id: 'normal-2' },
    { id: 'cron-2', source: 'cron' },
  ];

  test('defaults to showing cron sessions', () => {
    const result = splitSidebarSessions(sessions, new Set(['cron-1']), false);

    expect(result.pinned.map((session) => session.id)).toEqual(['cron-1']);
    expect(result.normal.map((session) => session.id)).toEqual(['normal-1', 'normal-2', 'cron-2']);
  });

  test('can hide cron sessions from pinned and recent groups', () => {
    const result = splitSidebarSessions(sessions, new Set(['cron-1', 'normal-2']), true);

    expect(result.pinned.map((session) => session.id)).toEqual(['normal-2']);
    expect(result.normal.map((session) => session.id)).toEqual(['normal-1']);
  });
});
