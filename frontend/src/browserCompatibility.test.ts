import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

const root = new URL('.', import.meta.url);

describe('older Chrome startup compatibility', () => {
  test('build keeps JavaScript and CSS output compatible with older Android Chrome', () => {
    const config = readFileSync(new URL('../../vite.config.ts', import.meta.url), 'utf8');
    expect(config).toMatch(/target:\s*['"]chrome83['"]/);
  });

  test('inline Markdown works without String.replaceAll on WebView 83', () => {
    const moduleUrl = new URL('./markdown.ts', import.meta.url).href;
    const input = '`code` [link](https://example.org)';
    const result = Bun.spawnSync([process.execPath, '-e', `String.prototype.replaceAll = undefined; const { markdownInline } = await import(${JSON.stringify(moduleUrl)}); process.stdout.write(markdownInline(${JSON.stringify(input)}));`]);
    expect(result.exitCode).toBe(0);
    const html = new TextDecoder().decode(result.stdout);
    expect(html).toContain('<code>code</code>');
    expect(html).toContain('href="https://example.org"');
  });

  test('runtime modules do not require native Array.at', () => {
    const offenders = readdirSync(root)
      .filter((name) => /\.tsx?$/.test(name) && !name.endsWith('.test.ts'))
      .filter((name) => /\.at\s*\(/.test(readFileSync(new URL(name, root), 'utf8')));
    expect(offenders).toEqual([]);
  });
});
