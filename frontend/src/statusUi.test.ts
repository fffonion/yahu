import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('ephemeral status UI', () => {
  test('does not render global status text in page headers or composer footer', () => {
    const app = source();
    const css = styles();

    expect(app).not.toContain('className="status-chip"');
    expect(app).not.toContain('<span>{props.status}</span>');
    expect(app).not.toContain('skill?.description || status');
    expect(app).not.toContain('status={status}');
    expect(css).not.toContain('status-chip');
  });
});
