import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('desktop compact chat toggle', () => {
  test('composer renders compact toggle immediately after thinking toggle', () => {
    const source = app();
    expect(source).toContain("desktopCompactMessages={desktopCompactMessages}");
    expect(source).toContain("setDesktopCompactMessages={setDesktopCompactMessages}");
    expect(source).toContain("reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`}");
    expect(source).toContain("desktop-compact-view-toggle ${props.desktopCompactMessages ? 'active' : ''}`}");
    expect(source.indexOf('reasoning-view-toggle')).toBeLessThan(source.indexOf('desktop-compact-view-toggle'));
    expect(source.indexOf('desktop-compact-view-toggle')).toBeLessThan(source.indexOf('tool-call-view-toggle'));
  });

  test('desktop compact mode uses mobile-like flat message styling with a turn block border', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("chat-main-panel ${props.desktopCompactMessages ? 'desktop-compact-chat' : ''}");
    expect(source).toContain('className="desktop-turn-block"');
    expect(source).toContain('buildDesktopTurnBlocks(turnDetailItems)');
    expect(styles).toContain('.desktop-compact-chat .msg-content{background:transparent;border:0;border-radius:0;box-shadow:none;padding:0}');
    expect(styles).toContain('.desktop-compact-chat .tool-card{background:transparent;border:0;border-radius:0;box-shadow:none;overflow:visible}');
    expect(styles).toContain('.desktop-compact-chat .desktop-turn-block{border:1px solid var(--border);border-radius:var(--radius-lg);');
    expect(styles).toContain('@media(max-width:760px){.desktop-compact-view-toggle{display:none!important}');
  });
});
