import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('settings project link', () => {
  test('shows the yahu GitHub project link with a GitHub icon', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('function GitHubIcon');
    expect(app).toContain('className="update-project-link-row"');
    expect(app).toContain('className="project-link"');
    expect(app).toContain('href="https://github.com/fffonion/yahu"');
    expect(app).toContain('aria-label="GitHub · fffonion/yahu"');
    expect(app).toContain('<GitHubIcon /> <span>GitHub · fffonion/yahu</span>');
    expect(app.indexOf("{t('settings.checkUpdate')}")).toBeLessThan(app.indexOf('className="update-project-link-row"'));
    expect(app.indexOf('className="update-project-link-row"')).toBeGreaterThan(app.indexOf("{t('settings.update')}</h3>"));
    expect(css).toContain('.project-link{width:max-content;display:inline-flex;align-items:center;gap:7px;');
    expect(css).toContain('text-decoration:none;font-size:13px}');
    expect(css).toContain('.update-project-link-row{margin-top:10px}');
    expect(css).toContain('.project-link svg{width:17px;height:17px}');
  });
});
