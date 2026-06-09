import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat reasoning display toggle', () => {
  test('composer has an icon-only quick toggle for reasoning/thinking visibility', () => {
    const source = app();
    expect(source).toContain("showReasoning={showReasoning}");
    expect(source).toContain("setShowReasoning={setShowReasoning}");
    expect(source).toContain("className={`icon-btn reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`}");
    expect(source).toContain("aria-pressed={props.showReasoning}");
  });

  test('assistant messages render reasoning only when the toggle is enabled', () => {
    const source = app();
    expect(source).toContain("<MessageView key={m.id} message={m} showReasoning={props.showReasoning} />");
    expect(source).toContain("message.reasoning && showReasoning");
    expect(source).toContain("className=\"msg-reasoning\"");
  });

  test('reasoning block and toggle have compact themed styles', () => {
    const styles = css();
    expect(styles).toContain('.reasoning-view-toggle.active');
    expect(styles).toContain('.msg-reasoning');
    expect(styles).toContain('.msg-reasoning pre');
  });
});
