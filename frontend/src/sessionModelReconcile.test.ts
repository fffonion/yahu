import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { preferNewerSessionModel } from './sessionModelReconcile';

describe('session model reconciliation', () => {
  test('uses the canonical current segment model before loading the old root detail', () => {
    const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
    expect(source).toContain('const currentModel = String(canonicalBody.current_model || \'\').trim()');
    expect(source).toContain('const resolvedDetail = canonicalModel ? { ...detail, ...canonicalModel, ended_at: undefined } : detail;');
    expect(source).toContain('preferNewerSessionModel(resolvedDetail, sessions.find((session) => session.id === detail.id))');
  });
  test('loads root detail without replacing newer list model in the app', () => {
    const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
    expect(source).toContain('preferNewerSessionModel(resolvedDetail, sessions.find((session) => session.id === detail.id))');
    expect(source).toContain('preferNewerSessionModel(resolvedDetail, s)');
  });
  test('keeps the latest list model when an old root detail loads later', () => {
    const detail = { id: 'root', model: 'minimax-m3', provider: 'minimax', ended_at: 100, message_count: 8 };
    const list = { id: 'root', model: 'gpt-6-sol', provider: 'openai-codex', last_active: 300, ended_at: null };
    expect(preferNewerSessionModel(detail, list)).toMatchObject({ model: 'gpt-6-sol', provider: 'openai-codex', ended_at: null });
  });

  test('accepts a fresh detail for an active session', () => {
    const detail = { id: 'live', model: 'gpt-6-sol', provider: 'openai-codex', ended_at: null };
    const list = { id: 'live', model: 'minimax-m3', provider: 'minimax', last_active: 300 };
    expect(preferNewerSessionModel(detail, list)).toMatchObject({ model: 'gpt-6-sol', provider: 'openai-codex' });
  });

  test('never borrows the model of a different session', () => {
    const detail = { id: 'side', model: 'minimax-m3', ended_at: 100 };
    const list = { id: 'root', model: 'gpt-6-sol', last_active: 300 };
    expect(preferNewerSessionModel(detail, list).model).toBe('minimax-m3');
  });
});
