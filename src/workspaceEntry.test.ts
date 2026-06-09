import { describe, expect, test } from 'bun:test';
import { workspaceEntryClickAction } from './workspaceEntry';

describe('workspaceEntryClickAction', () => {
  test('opens folders from a single row click', () => {
    expect(workspaceEntryClickAction('dir')).toBe('open');
  });

  test('opens files from a single row click', () => {
    expect(workspaceEntryClickAction('file')).toBe('open');
  });
});
