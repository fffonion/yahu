import { describe, expect, test } from 'bun:test';
import { isToolLikeMessage, shouldRenderMessage } from './messageVisibility';

describe('chat message visibility', () => {
  test('hides completed assistant messages that have no visible content', () => {
    expect(shouldRenderMessage({ role: 'assistant', content: '', pending: false })).toBe(false);
    expect(shouldRenderMessage({ role: 'assistant', content: '   \n\t', pending: false })).toBe(false);
  });

  test('keeps pending assistant placeholders and messages with content', () => {
    expect(shouldRenderMessage({ role: 'assistant', content: '', pending: true })).toBe(true);
    expect(shouldRenderMessage({ role: 'assistant', content: 'hello', pending: false })).toBe(true);
  });

  test('keeps reasoning-only assistant messages only when reasoning is visible', () => {
    const msg = { role: 'assistant', content: '', reasoning: 'hidden trace', pending: false };
    expect(shouldRenderMessage(msg, false)).toBe(false);
    expect(shouldRenderMessage(msg, true)).toBe(true);
  });

  test('tool call messages follow the tool-call visibility flag', () => {
    const msg = { role: 'tool', content: 'terminal output', pending: false };
    expect(shouldRenderMessage(msg)).toBe(true);
    expect(shouldRenderMessage(msg, false, true)).toBe(true);
    expect(shouldRenderMessage(msg, false, false)).toBe(false);
  });

  test('tool-like patch and terminal messages hide even when their role is not tool', () => {
    const patchMessage = { role: 'assistant', content: '<untrusted_tool_result source="patch">diff</untrusted_tool_result>', pending: false };
    const terminalMessage = { role: 'system', content: JSON.stringify({ tool_name: 'terminal', output: 'done' }), pending: false };
    const namedToolMessage = { role: 'assistant', content: 'plain output', toolName: 'functions.patch', pending: false };
    expect(isToolLikeMessage(patchMessage)).toBe(true);
    expect(isToolLikeMessage(terminalMessage)).toBe(true);
    expect(isToolLikeMessage(namedToolMessage)).toBe(true);
    expect(shouldRenderMessage(patchMessage, false, false)).toBe(false);
    expect(shouldRenderMessage(terminalMessage, false, false)).toBe(false);
    expect(shouldRenderMessage(namedToolMessage, false, false)).toBe(false);
  });
});
