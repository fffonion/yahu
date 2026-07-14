import { describe, expect, test } from 'bun:test';
import { compactSessionPreview, latestSessionPreviewFromMessages } from './sessionPreview';

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

  test('removes the gateway sender prefix from a user message preview', () => {
    expect(latestSessionPreviewFromMessages([
      { role: 'user', content: '[Alliumcepa Triplef|1698432746]\n消息本身' },
    ])).toBe('消息本身');
  });

  test('removes a leading bracketed sender prefix without a pipe', () => {
    expect(compactSessionPreview('[Alliumcepa Triplef]\n消息本身')).toBe('消息本身');
    expect(compactSessionPreview('[Alliumcepa Triplef] 消息本身')).toBe('消息本身');
  });

  test('keeps bracketed text that is not at the start', () => {
    expect(compactSessionPreview('消息 [保留]')).toBe('消息 [保留]');
  });
});
