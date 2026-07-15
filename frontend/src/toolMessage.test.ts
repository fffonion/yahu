import { describe, expect, test } from 'bun:test';
import { summarizeToolMessage } from './toolMessage';

describe('summarizeToolMessage', () => {
  test('builds a one-line tool summary from JSON without exposing raw JSON', () => {
    const summary = summarizeToolMessage(JSON.stringify({ tool_name: 'read_file', status: 'ok', path: '/tmp/a.txt', result: 'read 10 lines' }));
    expect(summary.title).toBe('read file');
    expect(summary.subtitle).toBe('read 10 lines');
    expect(summary.fields).toContainEqual({ key: 'path', value: '/tmp/a.txt' });
    expect(summary.result).toBe('read 10 lines');
  });

  test('preserves invocation parameters from tool content and external API fields', () => {
    const fromContent = summarizeToolMessage(JSON.stringify({ tool_name: 'read_file', arguments: { path: '/tmp/a.txt', limit: 20 }, result: 'read 20 lines' }));
    expect(fromContent.input).toEqual({ path: '/tmp/a.txt', limit: 20 });
    expect(fromContent.result).toBe('read 20 lines');
    expect(fromContent.fields).toContainEqual({ key: 'input', value: '{"path":"/tmp/a.txt","limit":20}' });

    const fromFallback = summarizeToolMessage('plain terminal output', 'functions.terminal', '{"command":"pwd","timeout":120}');
    expect(fromFallback.input).toEqual({ command: 'pwd', timeout: 120 });
    expect(fromFallback.result).toBe('plain terminal output');
  });

  test('uses untrusted tool result source as the displayed tool name', () => {
    const summary = summarizeToolMessage('<untrusted_tool_result source="browser_navigate">navigated</untrusted_tool_result>');
    expect(summary.title).toBe('browser_navigate');
    expect(summary.toolName).toBe('browser_navigate');
    expect(summary.subtitle).toBe('navigated');
    expect(summary.fields).toContainEqual({ key: 'source', value: 'browser_navigate' });
    expect(summary.fields).toContainEqual({ key: 'result', value: 'navigated' });
  });

  test('keeps the canonical tool name separate from the human display title for icon mapping', () => {
    const summary = summarizeToolMessage(JSON.stringify({ tool_name: 'read_file', status: 'ok', path: '/tmp/a.txt', result: 'read 10 lines' }));
    expect(summary.title).toBe('read file');
    expect(summary.toolName).toBe('read_file');
  });

  test('keeps skill tool fallback name when the result contains the skill name field', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ success: true, name: 'yahu', description: 'Use when developing yahu', content: '# Yahu' }),
      'functions.skill_view',
    );
    expect(summary.title).toBe('skill view');
    expect(summary.toolName).toBe('functions.skill_view');
  });

  test('falls back to structured output for non-json tool content', () => {
    const summary = summarizeToolMessage('plain output');
    expect(summary.title).toBe('tool');
    expect(summary.toolName).toBe('tool');
    expect(summary.fields).toEqual([{ key: 'output', value: 'plain output' }]);
  });

  test('uses the API-provided tool name when tool content is plain output', () => {
    const summary = summarizeToolMessage('plain terminal output', 'functions.terminal');
    expect(summary.title).toBe('terminal');
    expect(summary.toolName).toBe('functions.terminal');
    expect(summary.fields).toEqual([{ key: 'output', value: 'plain terminal output' }]);
  });

  test('terminal summary subtitle prefers the executed command over command output', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ output: 'validation=ok', exit_code: 0, error: null }),
      'terminal',
      { command: 'python3 validate.py --strict', timeout: 15 },
    );
    expect(summary.subtitle).toBe('python3 validate.py --strict');
    expect(summary.result).toBe('validation=ok');
    expect(summary.input).toEqual({ command: 'python3 validate.py --strict', timeout: 15 });
  });

  test('patch summary shows the source-relative path before the field count', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ success: true, resolved_path: '/home/wow/project/src/components/ToolCard.tsx', diff: 'changed', files_modified: ['/home/wow/project/src/components/ToolCard.tsx'] }),
      'functions.patch',
      { mode: 'replace', path: '/home/wow/project/src/components/ToolCard.tsx' },
    );
    expect(summary.subtitle).toBe('src/components/ToolCard.tsx · 4 fields');
  });

  test('patch summary falls back to just the filename when no src segment is in the path', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ success: true, resolved_path: '/home/wow/workspace/hermes-headless-webui/frontend/styles.css', diff: 'changed' }),
      'functions.patch',
      { mode: 'replace', path: '/home/wow/workspace/hermes-headless-webui/frontend/styles.css' },
    );
    expect(summary.subtitle).toBe('styles.css · 3 fields');
  });

  test('patch summary truncates long paths while keeping the filename intact', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ success: true, resolved_path: '/home/wow/project/src/a/b/c/d/e/f/g/ToolCard.tsx', diff: 'changed' }),
      'functions.patch',
      { mode: 'replace', path: '/home/wow/project/src/a/b/c/d/e/f/g/ToolCard.tsx' },
    );
    expect(summary.subtitle).toBe('src/…/g/ToolCard.tsx · 3 fields');
  });

  test('patch summary shows the error message instead of path and field count', () => {
    const summary = summarizeToolMessage(
      JSON.stringify({ success: false, error: 'new_string is not unique', resolved_path: '/home/wow/project/src/App.tsx' }),
      'functions.patch',
      { mode: 'replace', path: '/home/wow/project/src/App.tsx' },
    );
    expect(summary.subtitle).toBe('new_string is not unique');
  });
});
