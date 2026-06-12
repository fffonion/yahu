import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { dedupeVisibleChatMessages, isToolLikeMessage, renderableMessages, shouldRenderMessage } from './messageVisibility';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

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

  test('filters already-rendered tool frames out of the chat list when tool visibility is disabled', () => {
    const messages = [
      { role: 'user', content: 'run date', pending: false },
      { role: 'tool', content: 'Fri Jun 12', pending: false },
      { role: 'assistant', content: 'done', pending: false },
    ];
    expect(renderableMessages(messages, false, true).map((message) => message.role)).toEqual(['user', 'tool', 'assistant']);
    expect(renderableMessages(messages, false, false).map((message) => message.role)).toEqual(['user', 'assistant']);
  });

  test('does not merge assistant-shaped tool output into the final assistant answer', () => {
    const messages = [
      { id: 'u1', role: 'user', content: 'check files', pending: false },
      { id: 'a-tool', role: 'assistant', content: '<untrusted_tool_result source="search_files">matches</untrusted_tool_result>', pending: false },
      { id: 'a-final', role: 'assistant', content: 'I found the files.', pending: false },
    ];
    const deduped = dedupeVisibleChatMessages(messages);
    expect(deduped.map((message) => message.id)).toEqual(['u1', 'a-tool', 'a-final']);
    expect(renderableMessages(deduped, false, false).map((message) => message.id)).toEqual(['u1', 'a-final']);
  });

  test('keeps assistant tool-call placeholders from becoming final-answer dedupe targets', () => {
    const messages = [
      { id: 'a-tool-call', role: 'assistant', content: '', pending: false, toolCalls: [{ function: { name: 'web_extract' } }] },
      { id: 'tool-result', role: 'tool', content: 'tool output', pending: false },
      { id: 'a-final', role: 'assistant', content: 'final answer', pending: false },
    ];
    const deduped = dedupeVisibleChatMessages(messages);
    expect(isToolLikeMessage(messages[0])).toBe(true);
    expect(deduped.map((message) => message.id)).toEqual(['a-tool-call', 'tool-result', 'a-final']);
    expect(renderableMessages(deduped, false, false).map((message) => message.id)).toEqual(['a-final']);
  });

  test('ChatMain filters visible messages before mapping so hidden tool frames are unmounted', () => {
    const source = appSource();
    expect(source).toContain("import { dedupeVisibleChatMessages, isToolLikeMessage, renderableMessages } from './messageVisibility';");
    expect(source).toContain('const visibleMessages = renderableMessages<ChatMessage>(dedupeVisibleChatMessages<ChatMessage>(props.messages), props.showReasoning, props.showToolCalls);');
    expect(source).toContain('<MessageView message={m} showReasoning={props.showReasoning} assistantName={sessionModel || undefined} />');
    expect(source).not.toContain('if (!shouldRenderMessage(message, showReasoning, showToolCalls)) return null;');
    expect(source).not.toContain('showToolCalls?: boolean');
  });

  test('session changes clear old message data before loading the new window', () => {
    const source = appSource();
    expect(source).toContain('messageRequestRef.current += 1;\n    messagesRef.current = [];\n    hasOlderRef.current = false;\n    hasNewerRef.current = false;\n    pendingHistoryScrollAnchorRef.current = null;\n    setMessages([]);');
    expect(source).toContain("loadMessageWindow(activeSessionId, 'latest');");
  });

  test('visibility toggles preserve a message scroll anchor instead of shifting the viewport', () => {
    const source = appSource();
    expect(source).toContain("import { captureMessageScrollAnchor, restoreMessageScrollAnchor, type MessageScrollAnchor } from './chatScrollAnchor';");
    expect(source).toContain('const preserveChatScrollForVisibilityChange = (nextShowReasoning: boolean, nextShowToolCalls: boolean, apply: () => void) => {');
    expect(source).toContain('const nextVisibleIds = new Set(nextVisibleMessages.map((message) => String(message.id || \'\')).filter(Boolean));');
    expect(source).toContain('restoreMessageScrollAnchor(scroller, anchor);');
    expect(source).not.toContain('onClick={() => props.setShowReasoning(!props.showReasoning)}');
    expect(source).not.toContain('onClick={() => props.setShowToolCalls(!props.showToolCalls)}');
  });

  test('raw history window is larger than the rendered message target so hidden tool pages do not evict visible rows', () => {
    const source = appSource();
    expect(source).toContain('const RAW_MESSAGE_WINDOW = MESSAGE_WINDOW * 4;');
    expect(source).toContain("import { mergeMessageWindow } from './chatMessageWindow';");
    expect(source).toContain('limit: RAW_MESSAGE_WINDOW,');
    expect(source).toContain('const loadingMessagesRef = useRef(false);');
    expect(source).toContain("if (loadingMessagesRef.current && direction !== 'latest') return;");
  });
});
