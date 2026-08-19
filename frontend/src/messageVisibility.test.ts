import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { isAssistantToolPreludeMessage, isToolLikeMessage, renderableMessages, shouldRenderMessage, visibleChatMessages, withToolCallInputs } from './messageVisibility';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const transcriptSource = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');

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

  test('copies assistant tool-call arguments onto the matching tool result for collapsed summaries', () => {
    const messages = [
      { id: 'a-call', role: 'assistant', content: '', pending: false, toolCalls: [{ id: 'call_1', call_id: 'call_1', function: { name: 'terminal', arguments: '{"command":"date +%F","timeout":15}' } }] },
      { id: 'tool-result', role: 'tool', content: '{"output":"2026-07-07","exit_code":0}', pending: false, toolName: 'terminal', toolCallId: 'call_1' },
    ];
    const prepared = withToolCallInputs(messages);
    expect(prepared.find((message) => message.id === 'tool-result')?.toolInput).toEqual({ command: 'date +%F', timeout: 15 });
  });

  test('copies assistant tool-call arguments for web search query summaries', () => {
    const messages = [
      { id: 'web-call', role: 'assistant', content: '', pending: false, toolCalls: [{ id: 'call_web', call_id: 'call_web', function: { name: 'web_search', arguments: '{"query":"Hermes Agent documentation","limit":5}' } }] },
      { id: 'web-result', role: 'tool', content: '{"web":[{"title":"result"}]}', pending: false, toolName: 'web_search', toolCallId: 'call_web' },
    ];
    const prepared = withToolCallInputs(messages);
    expect(prepared.find((message) => message.id === 'web-result')?.toolInput).toEqual({ query: 'Hermes Agent documentation', limit: 5 });
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
    const prepared = withToolCallInputs(messages);
    expect(prepared.map((message) => message.id)).toEqual(['u1', 'a-tool', 'a-final']);
    expect(renderableMessages(prepared, false, false).map((message) => message.id)).toEqual(['u1', 'a-final']);
  });

  test('keeps final assistant answers after intervening tool results instead of moving them before tools', () => {
    const messages = [
      { id: 'u1', role: 'user', content: 'translate subtitles', pending: false },
      { id: 'a-progress', role: 'assistant', content: 'Translation started.', pending: false },
      { id: 'tool-write', role: 'tool', content: 'wrote subtitle batch', pending: false, toolName: 'write_file' },
      { id: 'tool-validate', role: 'tool', content: 'validation=ok', pending: false, toolName: 'terminal' },
      { id: 'a-final', role: 'assistant', content: 'Done. Uploaded the subtitles.', pending: false },
    ];
    const visible = renderableMessages(withToolCallInputs(messages), false, true);
    expect(visible.map((message) => message.id)).toEqual(['u1', 'a-progress', 'tool-write', 'tool-validate', 'a-final']);
  });

  test('assistant pre-tool text remains visible and non-tool while using compact styling', () => {
    const prelude = { id: 'a-pre-tool', role: 'assistant', content: 'Need to inspect the file first.', pending: false, toolCalls: [{ function: { name: 'read_file' } }] };
    expect(isAssistantToolPreludeMessage(prelude)).toBe(true);
    expect(isToolLikeMessage(prelude)).toBe(false);
    expect(shouldRenderMessage(prelude, false, false)).toBe(true);
    expect(renderableMessages([prelude, { id: 'tool-result', role: 'tool', content: 'file output', pending: false }], false, false).map((message) => message.id)).toEqual(['a-pre-tool']);
    const source = transcriptSource();
    const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(source).toContain("import { isAssistantToolPreludeMessage, isToolLikeMessage, visibleChatMessages } from './messageVisibility';");
    expect(source).toContain("const isToolPrelude = isAssistantToolPreludeMessage(message);");
    expect(source).toContain("${isToolPrelude ? ' tool-prelude' : ''}");
    expect(styles).toContain('.msg-row.tool-prelude .msg-content{color:var(--text)}');
    expect(styles).toContain('.msg-row.tool-prelude .msg-body{font-size:13px;line-height:1.45;color:var(--text)}');
  });

  test('hides empty assistant tool-call placeholders so they do not render blank tool result cards', () => {
    const placeholder = { id: 'a-tool-call', role: 'assistant', content: '', pending: false, toolCalls: [{ function: { name: 'web_extract' } }] };
    expect(isToolLikeMessage(placeholder)).toBe(true);
    expect(shouldRenderMessage(placeholder, false, true)).toBe(false);
  });

  test('keeps assistant tool-call placeholders from becoming final-answer dedupe targets', () => {
    const messages = [
      { id: 'a-tool-call', role: 'assistant', content: '', pending: false, toolCalls: [{ function: { name: 'web_extract' } }] },
      { id: 'tool-result', role: 'tool', content: 'tool output', pending: false },
      { id: 'a-final', role: 'assistant', content: 'final answer', pending: false },
    ];
    const prepared = withToolCallInputs(messages);
    expect(isToolLikeMessage(messages[0])).toBe(true);
    expect(prepared.map((message) => message.id)).toEqual(['a-tool-call', 'tool-result', 'a-final']);
    expect(renderableMessages(prepared, false, true).map((message) => message.id)).toEqual(['tool-result', 'a-final']);
    expect(renderableMessages(prepared, false, false).map((message) => message.id)).toEqual(['a-final']);
  });

  test('keeps an empty delegate call visible while waiting for its subagent result', () => {
    const messages = [
      { id: 'a-delegate', role: 'assistant', content: '', pending: false, toolCalls: [{ function: { name: 'delegate_task' } }] },
      { id: 'a-old', role: 'assistant', content: 'previous answer', pending: false },
    ];
    expect(renderableMessages(messages, false, true).map((message) => message.id)).toEqual(['a-delegate', 'a-old']);
    expect(renderableMessages(messages, false, false).map((message) => message.id)).toEqual(['a-old']);
  });

  test('ChatMain filters visible messages without content-level history dedupe before mapping', () => {
    const source = appSource();
    const transcript = transcriptSource();
    const messages = [
      { id: 'u1', role: 'user', content: 'run checks', pending: false },
      { id: 'a-progress', role: 'assistant', content: 'I will run it', pending: false, toolCalls: [{ id: 'call1', function: { arguments: '{"command":"bun test"}' } }] },
      { id: 'tool-write', role: 'tool', content: 'patched file', pending: false, toolCallId: 'call1' },
      { id: 'tool-validate', role: 'tool', content: 'tests passed', pending: false },
      { id: 'a-final', role: 'assistant', content: 'done', pending: false },
    ];
    expect(source).toContain("import { visibleChatMessages } from './messageVisibility';");
    expect(transcript).toContain("import { isAssistantToolPreludeMessage, isToolLikeMessage, visibleChatMessages } from './messageVisibility';");
    expect(visibleChatMessages(messages, false, true).map((message) => message.id)).toEqual(['u1', 'a-progress', 'tool-write', 'tool-validate', 'a-final']);
    expect(source).toContain('const visibleMessages = useMemo(() => visibleChatMessages<ChatMessage>(props.messages, props.showReasoning, props.showToolCalls), [props.messages, props.showReasoning, props.showToolCalls]);');
    expect(source).not.toContain('dedupeVisibleChatMessages');
    expect(source).toContain('<ChatTranscript');
    expect(transcript).toContain('<MessageView message={item.message} showReasoning={showReasoning} assistantName={assistantName} />');
    expect(transcript).not.toContain('if (!shouldRenderMessage(message, showReasoning, showToolCalls)) return null;');
    expect(transcript).toContain('showToolCalls: boolean');
  });

  test('session changes restore cached message data before refreshing the latest window', () => {
    const source = appSource();
    expect(source).toContain('const sessionMessageCacheRef = useRef<Map<string, SessionMessageCache>>(new Map());');
    expect(source).toContain('const restored = restoreCachedMessageWindow(activeSessionId);');
    expect(source).toContain('setMessages(cached.messages);');
    expect(source).toContain('if (!restored) {\n      messagesRef.current = [];');
    expect(source).toContain("loadMessageWindow(activeSessionId, 'latest', restored ? undefined : savedAnchorId);");
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
    expect(source).toContain("import { mergeMessageWindow, sortMessagesInDisplayOrder } from './chatMessageWindow';");
    expect(source).toContain('limit: RAW_MESSAGE_WINDOW,');
    expect(source).toContain('const loadingMessagesRef = useRef(false);');
    expect(source).toContain("if (loadingMessagesRef.current && direction !== 'latest') return;");
  });
});
