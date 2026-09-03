import { describe, expect, test } from 'bun:test';
import { splitSidebarSessions, reorderPinnedIds } from './sessionListFilter';

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

  test('keeps pinned sessions in the explicit pinned order instead of API order', () => {
    const result = splitSidebarSessions([sessions[0], sessions[1], sessions[2], sessions[3]], new Set(['normal-2', 'cron-1', 'normal-1']));

    expect(result.pinned.map((session) => session.id)).toEqual(['normal-2', 'cron-1', 'normal-1']);
  });

  test('moves a pinned session before the drop target', () => {
    expect(Array.from(reorderPinnedIds(new Set(['one', 'two', 'three']), 'three', 'one'))).toEqual(['three', 'one', 'two']);
    expect(Array.from(reorderPinnedIds(new Set(['one', 'two', 'three']), 'one', 'three'))).toEqual(['two', 'one', 'three']);
  });
});
