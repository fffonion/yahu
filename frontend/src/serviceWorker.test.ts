import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const sw = () => readFileSync(new URL('../public/sw.js', import.meta.url), 'utf8');

describe('service worker cache updates', () => {
  test('does not pin the app shell root in an old cache', () => {
    const source = sw();
    expect(source).toContain("const CACHE = 'yahu-v2';");
    expect(source).toContain("const PRECACHE = ['/manifest.json', '/icon.svg', '/icon-192.png', '/icon-512.png'];");
    expect(source).not.toContain("const PRECACHE = ['/',");
    expect(source).toContain("if (request.mode === 'navigate' || url.pathname === '/')");
    expect(source).toContain("e.respondWith(fetch(request));");
  });
});
