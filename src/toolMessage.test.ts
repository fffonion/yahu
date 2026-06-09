import { describe, expect, test } from 'bun:test';
import { summarizeToolMessage } from './toolMessage';

describe('summarizeToolMessage', () => {
  test('builds a one-line tool summary from JSON without exposing raw JSON', () => {
    const summary = summarizeToolMessage(JSON.stringify({ tool_name: 'read_file', status: 'ok', path: '/tmp/a.txt', result: 'read 10 lines' }));
    expect(summary.title).toBe('read file');
    expect(summary.subtitle).toBe('read 10 lines');
    expect(summary.fields).toContainEqual({ key: 'path', value: '/tmp/a.txt' });
  });

  test('uses untrusted tool result source as the displayed tool name', () => {
    const summary = summarizeToolMessage('<untrusted_tool_result source="browser_navigate">navigated</untrusted_tool_result>');
    expect(summary.title).toBe('browser_navigate');
    expect(summary.subtitle).toBe('navigated');
    expect(summary.fields).toContainEqual({ key: 'source', value: 'browser_navigate' });
    expect(summary.fields).toContainEqual({ key: 'result', value: 'navigated' });
  });

  test('falls back to structured output for non-json tool content', () => {
    const summary = summarizeToolMessage('plain output');
    expect(summary.title).toBe('tool');
    expect(summary.fields).toEqual([{ key: 'output', value: 'plain output' }]);
  });
});
