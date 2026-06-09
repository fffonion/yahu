import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat history infinite scroll', () => {
  test('history pagination is driven by scroll positions, not visible load-more buttons', () => {
    const source = app();
    expect(source).toContain('if (el.scrollTop < 80 && props.hasOlder && !props.loadingMessages) props.loadMessageWindow(props.activeSessionId, \'older\')');
    expect(source).toContain('if (el.scrollHeight - el.scrollTop - el.clientHeight < 80 && props.hasNewer && !props.loadingMessages) props.loadMessageWindow(props.activeSessionId, \'newer\')');
    expect(source).not.toContain('Load earlier messages');
    expect(source).not.toContain('Load newer messages');
    expect(source).toContain('className="history-loading"');
  });

  test('history loading indicator is non-interactive', () => {
    const styles = css();
    expect(styles).toContain('.history-loading');
    expect(styles).toContain('pointer-events:none');
  });
});
