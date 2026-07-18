import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('session sidebar source badges', () => {
  test('session rows show cron and CLI badges but no default circle marker', () => {
    const source = app();

    expect(source).toContain("source?: string");
    expect(source).toContain("session.source === 'cron'");
    expect(source).toContain('<CalendarClock />');
    expect(source).toContain("session.source === 'cli'");
    expect(source).toContain('<Terminal />');
    expect(source).not.toContain('Circle,');
    expect(source).not.toContain('<Circle />');
  });

  test('normal session rows do not reserve the old left marker column', () => {
    const styles = css();

    expect(styles).toContain('.session-item{width:100%;border:1px solid transparent;border-radius:16px;background:transparent;display:grid;grid-template-columns:minmax(0,1fr) 28px;');
    expect(styles).toContain('.session-item.has-leading-icon{grid-template-columns:22px minmax(0,1fr) 28px}');
  });
});
