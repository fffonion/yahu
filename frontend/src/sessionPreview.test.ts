import { describe, expect, test } from 'bun:test';
import { latestSessionPreviewFromMessages } from './sessionPreview';

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
});
