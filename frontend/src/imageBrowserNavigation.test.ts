import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { nextImageAfterRemoval, nextImageForPreload } from './imageBrowserNavigation';

const img = (filename: string) => ({ filename });
const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('image browser deletion navigation', () => {
  test('selects the immediate next image after deleting the open preview without skipping one', () => {
    const images = [img('a.png'), img('b.png'), img('c.png'), img('d.png')];
    expect(nextImageAfterRemoval(images, ['b.png'], 'b.png')?.filename).toBe('c.png');
  });

  test('selects the previous image when deleting the last open preview', () => {
    const images = [img('a.png'), img('b.png'), img('c.png')];
    expect(nextImageAfterRemoval(images, ['c.png'], 'c.png')?.filename).toBe('b.png');
  });

  test('returns null only when no images remain after deleting the open preview', () => {
    expect(nextImageAfterRemoval([img('a.png')], ['a.png'], 'a.png')).toBeNull();
  });

  test('keeps the current preview when deletion does not include it', () => {
    const images = [img('a.png'), img('b.png'), img('c.png')];
    expect(nextImageAfterRemoval(images, ['a.png'], 'b.png')?.filename).toBe('b.png');
  });

  test('modal delete precomputes the replacement before the server delete can emit an event', () => {
    const source = app();
    expect(source).toContain('const nextModalAfterDelete = modal && names.includes(modal.filename) ? nextImageAfterRemoval(imagesRef.current, names, modal.filename) : null;');
    expect(source).toContain('removeImages(names, nextModalAfterDelete);');
  });

  test('preloads only the immediate next modal image', () => {
    const images = [img('a.png'), img('b.png'), img('c.png')];
    expect(nextImageForPreload(images, 'a.png')?.filename).toBe('b.png');
    expect(nextImageForPreload(images, 'b.png')?.filename).toBe('c.png');
    expect(nextImageForPreload(images, 'c.png')).toBeNull();
    expect(nextImageForPreload(images, 'missing.png')).toBeNull();
  });

  test('modal image preloader removes the previous preload link before installing the next one', () => {
    const source = app();
    expect(source).toContain('modalPreloadLinkRef');
    expect(source).toContain("link.rel = 'preload';");
    expect(source).toContain("link.as = 'image';");
    expect(source).toContain('nextImageForPreload(images, modal.filename)');
    expect(source).toContain('modalPreloadLinkRef.current?.remove();');
    expect(source).toContain('return removeModalPreloadLink;');
  });
});
