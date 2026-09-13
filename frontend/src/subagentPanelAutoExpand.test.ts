import { describe, expect, test } from 'bun:test';
import { shouldAutoExpandSubagentPanel } from './subagentProgress';

describe('subagent panel auto-expansion', () => {
  test('expands once when a session has multiple projected children', () => {
    expect(shouldAutoExpandSubagentPanel('ffc', '', 7)).toBe(true);
    expect(shouldAutoExpandSubagentPanel('ffc', 'ffc', 7)).toBe(false);
    expect(shouldAutoExpandSubagentPanel('ffc', '', 1)).toBe(false);
    expect(shouldAutoExpandSubagentPanel('', '', 7)).toBe(false);
  });
});
