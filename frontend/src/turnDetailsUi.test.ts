import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('turn detail fold UI', () => {
  test('chat history renders final-turn intermediate tool and thinking rows inside a second-level collapsed details group', () => {
    const source = app();
    expect(source).toContain("buildTurnDetailItems");
    expect(source).toContain("<TurnDetailGroup");
    expect(source).toContain('className="turn-detail-group"');
    expect(source).toContain('className="turn-detail-summary"');
    expect(source).toContain('aria-label="Turn tools and thinking"');
  });

  test('outer turn detail group has compact collapsed styling separate from inner tool and reasoning folds', () => {
    const styles = css();
    expect(styles).toContain('.turn-detail-group');
    expect(styles).toContain('.turn-detail-summary');
    expect(styles).toContain('.turn-detail-group:not([open]) .turn-detail-body{display:none}');
    expect(styles).toContain('.turn-detail-body .tool-summary');
    expect(styles).toContain('.turn-detail-body .msg-reasoning');
  });
});
