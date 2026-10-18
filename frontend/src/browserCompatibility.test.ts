import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

const root = new URL('.', import.meta.url);

describe('older Chrome startup compatibility', () => {
  test('build keeps JavaScript and CSS output compatible with older Android Chrome', () => {
    const config = readFileSync(new URL('../../vite.config.ts', import.meta.url), 'utf8');
    expect(config).toMatch(/target:\s*['"]chrome87['"]/);
  });

  test('runtime modules do not require native Array.at', () => {
    const offenders = readdirSync(root)
      .filter((name) => /\.tsx?$/.test(name) && !name.endsWith('.test.ts'))
      .filter((name) => /\.at\s*\(/.test(readFileSync(new URL(name, root), 'utf8')));
    expect(offenders).toEqual([]);
  });
});
