import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => [readFileSync(new URL('./App.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./chatMessage.ts', import.meta.url), 'utf8')].join('\n');

describe('cross-platform session streaming watcher', () => {
  test('watch events merge into chat messages, replace streaming updates, and show a pending assistant card for remote user turns', () => {
    const app = source();

    expect(app).toContain("const OTHER_PLATFORM_PENDING_ID = 'other-platform-pending';");
    expect(app).toContain('function mergeWatchedMessage(prev: ChatMessage[], msg: ChatMessage): ChatMessage[]');
    expect(app).toContain('if (prev.some((m) => m.id === msg.id)) return prev.map((m) => m.id === msg.id ? { ...m, ...msg } : m);');
    expect(app).toContain("msg.role === 'user'");
    expect(app).toContain("msg.role === 'assistant'");
    expect(app).toContain('findCurrentTurnPersistedAssistantIndex(prev)');
    expect(app).toContain('m.content === msg.content || !isLocalStreamAssistant(msg)');
    expect(app).toContain('function isLocalStreamTool(message: ChatMessage)');
    expect(app).toContain('sameFinalIdx >= 0');
    expect(app).toContain('function findUnreconciledLocalAssistantIndex(prev: ChatMessage[])');
    expect(app).toContain('const turnLocalStreamIdx = findUnreconciledLocalAssistantIndex(prev);');
    expect(app).toContain('i === turnLocalStreamIdx ? { ...m, ...msg, pending: false } : m');
    expect(app).toContain('const prev = messagesRef.current;');
    expect(app).toContain('const next = sortMessagesInDisplayOrder(mergeWatchedMessage(prev, msg));');
    expect(app).toContain('messagesRef.current = next;');
    expect(app).toContain('computeNewMessageMarker(previousVisible, nextVisible, newMessageBoundaryIdRef.current)');
    expect(app).toContain('newMessageBoundaryId={newMessageBoundaryId}');
    expect(app).toContain('findNewMessageSplitIndex(visibleMessages, newMessageBoundaryId || undefined)');
  });

  test('persisted user events preserve the local turn anchor so open details stay mounted', () => {
    const app = source();
    expect(app).toContain('i === existing ? { ...m, ...msg, id: m.id } : m');

  });

  test('persisted tool events replace local streaming tool cards in place', () => {
    const app = source();
    expect(app).toContain("return prev.map((m) => isLocalStreamTool(m) && (m.toolName || '') === (msg.toolName || '') ? { ...m, ...msg, pending: false } : m);");

  });

  test('coalesces expensive context-window refreshes during rapid watch updates', () => {
    const app = source();
    expect(app).toContain('const contextWindowRefreshTimerRef = useRef<number | null>(null);');
    expect(app).toContain('const scheduleContextWindowSnapshot = useCallback((sessionId: string) => {');
    expect(app).toContain('scheduleContextWindowSnapshot(watchedSessionId);');
    expect(app).not.toContain('loadContextWindowSnapshot(watchedSessionId);');
  });
});
