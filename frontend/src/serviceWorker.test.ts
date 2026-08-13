import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const sw = () => readFileSync(new URL('../public/sw.js', import.meta.url), 'utf8');
const manifest = () => readFileSync(new URL('../public/manifest.json', import.meta.url), 'utf8');

describe('service worker cache updates', () => {
  test('does not pin the app shell root in an old cache', () => {
    const source = sw();
    expect(source).toContain("const CACHE = 'yahu-v4';");
    expect(source).toContain("const PRECACHE = ['/manifest.json', '/icon.svg', '/icon-192.png', '/icon-512.png'];");

    expect(source).toContain("if (request.mode === 'navigate' || url.pathname === '/')");
    expect(source).toContain("e.respondWith(fetch(request));");
  });

  test('does not cache session API responses', () => {
    const source = sw();
    expect(source).toContain("url.pathname.startsWith('/sessions')");
  });

  test('does not cache frontend assets from a stale cache', () => {
    const source = sw();
    expect(source).toContain("if (url.pathname.startsWith('/assets/'))");
    expect(source).toContain('e.respondWith(fetch(request));');

  });

  test('launches installed shortcuts in fullscreen with standalone fallback', () => {
    const appManifest = JSON.parse(manifest());
    expect(appManifest.display_override).toEqual(['fullscreen', 'standalone']);
    expect(appManifest.display).toBe('fullscreen');
  });
});
