import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const transcript = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('streaming character fade', () => {
  test('keeps the streaming header to the model name and animated dots', () => {
    expect(transcript()).toContain('stream-dots');
  });

  test('renders pending assistant text as stable grapheme spans with a fade-in animation', () => {
    expect(transcript()).toContain("from './streamGraphemes'");
    expect(transcript()).toContain('stream-fade-text');
    expect(transcript()).toContain('stream-fade-char');
    expect(transcript()).toContain('markdownText(text)');
    expect(styles()).toContain('.stream-fade-char{');
    expect(styles()).toContain('@keyframes stream-char-fade');
  });
});
