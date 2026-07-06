import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('session boundary times', () => {
  test('chat history page boundary timestamps update the active session header source', () => {
    const app = source();
    expect(app).toContain('type MessagePage = { data: any[]; total: number; has_older: boolean; has_newer: boolean; started_at?: number | string; last_active?: number | string }');
    expect(app).toContain('const updateSessionBoundaryTimes = useCallback((sessionId: string, page: MessagePage) => {');
    expect(app).toContain('if (page.started_at !== undefined) patch.started_at = page.started_at;');
    expect(app).toContain('if (page.last_active !== undefined) patch.last_active = page.last_active;');
    expect(app).toContain('updateSessionBoundaryTimes(sessionId, page);');
  });

  test('streaming local assistant updates carry timestamps for live latest header changes', () => {
    const app = source();
    expect(app).toContain("const assistantMsg: ChatMessage = { id: assistantId, role: 'assistant', content: '', pending: true, timestamp: Date.now() / 1000, model: sessionModel, provider: sessionProvider };");
    expect(app).toContain('{ ...m, content: text, pending: true, timestamp: Date.now() / 1000 }');
    expect(app).toContain('{ ...m, pending: false, content: finalText || m.content, reasoning: reasoningText || m.reasoning, timestamp: Date.now() / 1000 }');
  });
});
