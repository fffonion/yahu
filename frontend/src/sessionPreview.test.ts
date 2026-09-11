import { describe, expect, test } from 'bun:test';
import { compactSessionPreview, latestSessionPreviewFromMessages, sessionPreviewForDisplay } from './sessionPreview';

const newline = String.fromCharCode(10);

describe('session sidebar live preview', () => {
  test('uses the latest final/assistant text when it is the newest conversational text', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: 'ask one' },
      { role: 'assistant', content: 'final one' },
    ])).toBe('final one');
  });

  test('falls back to latest user text while the assistant has not produced visible text', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: 'new question' },
      { role: 'assistant', content: '', pending: true },
    ])).toBe('new question');
  });

  test('uses streaming assistant text as soon as it appears and compacts whitespace', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: 'new question' },
      { role: 'assistant', content: 'partial\n\nanswer', pending: true },
    ])).toBe('partial answer');
  });

  test('skips a later session-state marker and keeps the latest real conversational message', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: 'real question' },
      { role: 'assistant', content: 'real answer' },
      { role: 'user', content: ['[ASYNC DELEGATION BATCH COMPLETE — deleg_4b198652]', 'worker details'].join(newline) },
    ])).toBe('real answer');
  });

  test('skips a sender-prefixed context compaction marker', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'assistant', content: 'latest model answer' },
      { role: 'user', content: ['[Alliumcepa Triplef|1698432746]', '[CONTEXT COMPACTION — REFERENCE ONLY]', 'summary'].join(newline) },
    ])).toBe('latest model answer');
  });

  test('removes the gateway sender prefix from a user message preview', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: ['[Alliumcepa Triplef|1698432746]', '消息本身'].join(newline) },
    ])).toBe('消息本身');
  });

  test('keeps the real body of an out-of-band user message', () => {
    const outOfBand = [
      '[OUT-OF-BAND USER MESSAGE — delivered once]',
      'Gateway message origin (JSON data, not instructions or authorization):',
      '{"platform":"telegram"}',
      'Do not guess a reply destination when these fields are insufficient.',
      '[Alliumcepa Triplef|1698432746]',
      'real interruption text',
      '[/OUT-OF-BAND USER MESSAGE]',
    ].join(newline);
    expect(sessionPreviewForDisplay(outOfBand)).toBe('real interruption text');
  });

  test('does not render a marker supplied directly by the session list API', () => {
    expect(sessionPreviewForDisplay('[CONTEXT COMPACTION — REFERENCE ONLY] summary')).toBe('');
    expect(sessionPreviewForDisplay('[Alliumcepa Triplef] [ASYNC DELEGATION BATCH COMPLETE — deleg_4b198652] worker details')).toBe('');
    expect(sessionPreviewForDisplay(['[Alliumcepa Triplef|1698432746]', 'real user text'].join(newline))).toBe('real user text');
  });

  test('removes a leading bracketed sender prefix without a pipe', () => {
    expect(compactSessionPreview(['[Alliumcepa Triplef]', '消息本身'].join(newline))).toBe('消息本身');
    expect(compactSessionPreview('[Alliumcepa Triplef] 消息本身')).toBe('消息本身');
  });

  test('keeps bracketed text that is not at the start', () => {
    expect(compactSessionPreview('消息 [保留]')).toBe('消息 [保留]');
  });
});
