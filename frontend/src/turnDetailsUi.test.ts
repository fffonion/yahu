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
    expect(source).toContain('aria-label={t(\'chat.details\')}');
    expect(source).toContain('<span className="turn-detail-title">{t(\'chat.details\')}</span>');
    expect(source).not.toContain('Tools & thinking');
    expect(source).not.toContain('Turn tools and thinking');
  });

  test('outer turn detail summary exposes a visible translated expand control', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="turn-detail-toggle"');
    expect(source).toContain('<span className="turn-detail-toggle-label">{t(\'chat.expandDetails\')}</span>');
    expect(source).not.toContain('<span className="turn-detail-toggle-label">Expand</span>');
    expect(styles).toContain('.turn-detail-toggle{display:inline-grid;grid-template-columns:14px auto;');
    expect(source).toContain('onToggle={(event) => { const detail = event.currentTarget; detail.style.setProperty(\'--turn-detail-toggle-label\', `"${detail.open ? t(\'chat.collapseDetails\') : t(\'chat.expandDetails\')}"`); }}');
    expect(styles).toContain('.turn-detail-group[open] .turn-detail-toggle-label::before{content:var(--turn-detail-toggle-label)}');
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
