import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { normalizeChatMessage } from './chatMessage';

describe('history coverage gap', () => {
  test('normalizes backend gap metadata', () => {
    const message = normalizeChatMessage({
      id: -8_000_000_000_000,
      role: 'system',
      content: 'History coverage gap',
      history_gap: { after: 5, before: 90_000 },
    }, 'fallback');

    expect(message.historyGap).toEqual({ after: 5, before: 90_000 });
  });

  test('renders a dedicated localized boundary instead of a system chat bubble', () => {
    const transcript = readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
    const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(transcript).toContain('function HistoryCoverageGap');
    expect(transcript).toContain("t('chat.historyCoverageGap')");
    expect(styles).toContain('.history-coverage-gap{width:100%;max-width:920px;align-self:flex-start;display:flex;');
    expect(styles).toContain('max-width:min(70%,560px)');
  });
});
