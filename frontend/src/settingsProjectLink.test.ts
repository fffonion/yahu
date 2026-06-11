import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('settings project link', () => {
  test('shows the yahu GitHub project link with a GitHub icon', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('function GitHubIcon');
    expect(app).toContain('className="project-link"');
    expect(app).toContain('href="https://github.com/fffonion/yahu"');
    expect(app).toContain('aria-label="GitHub project"');
    expect(app).toContain('<GitHubIcon /> <span>GitHub · fffonion/yahu</span>');
    expect(css).toContain('.project-link{width:max-content;display:inline-flex;align-items:center;gap:7px;');
    expect(css).toContain('.project-link svg{width:17px;height:17px}');
  });
});
