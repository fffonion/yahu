import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { shouldRenderMessage, visibleChatMessages, withToolCallReasoning } from './messageVisibility';

const transcript = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('reasoning attached to tool-call turns', () => {
  test('a tool-call turn with no text stays a hidden placeholder', () => {
    const message = {
      role: 'assistant',
      content: '',
      pending: false,
      reasoning: 'plan the next tool call',
      toolCalls: [{ id: 'call_1', call_id: 'call_1', function: { name: 'terminal', arguments: '{"command":"date"}' } }],
    };
    expect(shouldRenderMessage(message, true, true)).toBe(false);
    expect(shouldRenderMessage(message, false, true)).toBe(false);
  });

  test('copies the turn thinking onto the matching tool result', () => {
    const messages = [
      { id: 'a-call', role: 'assistant', content: '', pending: false, reasoning: 'I should check the branch first.', toolCalls: [{ id: 'call_1', call_id: 'call_1', function: { name: 'terminal', arguments: '{"command":"git status"}' } }] },
      { id: 'tool-result', role: 'tool', content: '{"output":"on branch main","exit_code":0}', pending: false, toolName: 'terminal', toolCallId: 'call_1' },
    ];
    const prepared = withToolCallReasoning(messages);
    expect(prepared.find((message) => message.id === 'tool-result')?.toolReasoning).toBe('I should check the branch first.');
    // The assistant placeholder itself is unchanged.
    expect((prepared[0] as { toolReasoning?: string }).toolReasoning).toBeUndefined();
  });

  test('leaves tool results without a matching thinking turn untouched', () => {
    const messages = [
      { id: 'a-call', role: 'assistant', content: 'Running it.', pending: false, toolCalls: [{ id: 'call_9', call_id: 'call_9', function: { name: 'terminal', arguments: '{}' } }] },
      { id: 'unmatched', role: 'tool', content: '{"output":"ok"}', pending: false, toolName: 'terminal', toolCallId: 'call_other' },
    ];
    const prepared = withToolCallReasoning(messages);
    expect(prepared).toBe(messages);
  });

  test('the tool card shows the copied thinking in the shared reasoning block', () => {
    const source = transcript();
    const toolStart = source.indexOf('const ToolMessageView = React.memo(');
    const toolEnd = source.indexOf('function roleName(', toolStart);
    const toolSource = source.slice(toolStart, toolEnd);
    expect(toolSource).toContain('{message.toolReasoning && showReasoning && <details className="msg-reasoning msg-reasoning-collapsed"');
    expect(toolSource).toContain('msg-reasoning-body');
    expect(toolSource).toContain('markdownText(message.toolReasoning)');
    expect(toolSource).not.toContain('<pre>{message.toolReasoning}</pre>');
    // The tool summary button and result content still render.
    expect(toolSource).toContain('className="tool-summary"');
  });

  test('visible chat messages carry the tool-turn thinking', () => {
    const messages = [
      { id: 'a-call', role: 'assistant', content: '', pending: false, reasoning: 'think', toolCalls: [{ id: 'c1', call_id: 'c1', function: { name: 'terminal', arguments: '{}' } }] },
      { id: 't1', role: 'tool', content: '{"output":"ok"}', pending: false, toolName: 'terminal', toolCallId: 'c1' },
    ];
    const visible = visibleChatMessages(messages, true, true);
    expect(visible.map((message) => message.id)).toEqual(['t1']);
    expect(visible[0].toolReasoning).toBe('think');
  });

  test('the tool-card reasoning block keeps a tidy margin inside the card', () => {
    expect(css()).toContain('.tool-card .msg-reasoning{margin:0 0 8px}');
  });
});
