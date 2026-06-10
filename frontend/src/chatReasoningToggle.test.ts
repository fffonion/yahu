import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
function cssRule(styles: string, selector: string) {
  const start = styles.indexOf(`${selector}{`);
  if (start < 0) return '';
  const end = styles.indexOf('}', start);
  return end >= 0 ? styles.slice(start, end + 1) : '';
}

describe('chat reasoning display toggle', () => {
  test('composer has an icon-only quick toggle for reasoning/thinking visibility', () => {
    const source = app();
    expect(source).toContain("showReasoning={showReasoning}");
    expect(source).toContain("setShowReasoning={setShowReasoning}");
    expect(source).toContain("className={`icon-btn reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`}");
    expect(source).toContain("aria-pressed={props.showReasoning}");
    expect(source).toContain('<Lightbulb /></button>');
    expect(source).not.toContain('reasoning-view-toggle ${props.showReasoning ? \'active\' : \'\'}`} aria-pressed={props.showReasoning} aria-label={props.showReasoning ? t(\'chat.hideThinking\') : t(\'chat.showThinking\')} title={props.showReasoning ? t(\'chat.hideThinking\') : t(\'chat.showThinking\')} onClick={() => props.setShowReasoning(!props.showReasoning)}><Eye /></button>');
  });

  test('assistant messages render reasoning only when the toggle is enabled', () => {
    const source = app();
    expect(source).toContain("<MessageView message={m} showReasoning={props.showReasoning} />");
    expect(source).toContain("message.reasoning && showReasoning");
    expect(source).toContain("className=\"msg-reasoning\"");
  });

  test('reasoning block and toggle have compact themed styles', () => {
    const styles = css();
    expect(styles).toContain('.reasoning-view-toggle{justify-content:center;padding:0}');
    expect(styles).toContain('.reasoning-view-toggle.active');
    expect(styles).toContain('.msg-reasoning');
    expect(styles).toContain('.msg-reasoning pre');
    expect(cssRule(styles, '.msg-reasoning')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning>span')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning pre')).not.toContain('var(--accent');
    expect(cssRule(styles, '.msg-reasoning')).toContain('var(--surface-2)');
  });
});
