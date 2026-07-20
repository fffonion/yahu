import { describe, expect, test } from 'bun:test';
import { splitSidebarSessions } from './sessionListFilter';

describe('session list source filter', () => {
  const sessions = [
    { id: 'normal-1', source: 'telegram' },
    { id: 'cron-1', source: 'cron' },
    { id: 'cli-1', source: 'cli' },
    { id: 'normal-2' },
    { id: 'cron-2', source: 'cron' },
    { id: 'cli-2', source: 'cli' },
    { id: 'tui-1', source: 'tui' },
  ];

  test('defaults to showing cron and CLI sessions', () => {
    const result = splitSidebarSessions(sessions, new Set(['cron-1', 'cli-1']));

    expect(result.pinned.map((session) => session.id)).toEqual(['cron-1', 'cli-1']);
    expect(result.normal.map((session) => session.id)).toEqual(['normal-1', 'normal-2', 'cron-2', 'cli-2', 'tui-1']);
  });

  test('source filtering is left to the backend before pagination', () => {
    const result = splitSidebarSessions(sessions, new Set(['cron-1', 'cli-1', 'normal-2']));

    expect(result.pinned.map((session) => session.id)).toEqual(['cron-1', 'cli-1', 'normal-2']);
    expect(result.normal.map((session) => session.id)).toEqual(['normal-1', 'cron-2', 'cli-2', 'tui-1']);
  });
});
