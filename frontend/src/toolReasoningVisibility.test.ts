import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { shouldRenderMessage } from './messageVisibility';

const transcript = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');

describe('reasoning attached to tool-call turns', () => {
  test('a tool-call turn with thinking and no text is kept only when reasoning is visible', () => {
    const message = {
      role: 'assistant',
      content: '',
      pending: false,
      reasoning: 'plan the next tool call',
      toolCalls: [{ id: 'call_1', call_id: 'call_1', function: { name: 'terminal', arguments: '{"command":"date"}' } }],
    };
    expect(shouldRenderMessage(message, true, true)).toBe(true);
    expect(shouldRenderMessage(message, false, true)).toBe(false);
    // Tool output hidden means the turn (and its thinking) stays hidden too.
    expect(shouldRenderMessage(message, true, false)).toBe(false);
  });

  test('a tool-call turn without thinking still collapses to nothing', () => {
    const message = {
      role: 'assistant',
      content: '',
      pending: false,
      toolCalls: [{ id: 'call_2', call_id: 'call_2', function: { name: 'terminal', arguments: '{"command":"ls"}' } }],
    };
    expect(shouldRenderMessage(message, true, true)).toBe(false);
    expect(shouldRenderMessage(message, false, true)).toBe(false);
  });

  test('the tool card renders the same markdown reasoning block as assistant messages', () => {
    const source = transcript();
    expect(source).toContain("function ToolMessageView({ message, showReasoning = false, suppressMessageAnchor = false }");
    const toolStart = source.indexOf('const ToolMessageView = React.memo(');
    const toolEnd = source.indexOf('function roleName(', toolStart);
    const toolSource = source.slice(toolStart, toolEnd);
    expect(toolSource).toContain('{message.reasoning && showReasoning && <details className="msg-reasoning msg-reasoning-collapsed"');
    expect(toolSource).toContain('msg-reasoning-body');
    expect(toolSource).toContain('markdownText(message.reasoning)');
    expect(toolSource).not.toContain('<pre>{message.reasoning}</pre>');
  });

  test('MessageView forwards the reasoning flag to the tool card', () => {
    const source = transcript();
    expect(source).toContain('if (isToolLikeMessage(message)) return <ToolMessageView message={message} showReasoning={showReasoning} suppressMessageAnchor={suppressMessageAnchor} />;');
  });
});
