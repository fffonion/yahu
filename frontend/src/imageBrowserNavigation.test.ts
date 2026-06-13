import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { nextImageAfterRemoval } from './imageBrowserNavigation';

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
});
