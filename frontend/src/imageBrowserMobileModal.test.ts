import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('mobile image modal metadata controls', () => {
  test('mobile modal has an explicit metadata toggle button and hidden floating panel by default', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('modalMetadataOpen');
    expect(source).toContain('setModalMetadataOpen');
    expect(source).toContain('aria-label="Metadata"');
    expect(source).toContain('metadata-open');
    expect(styles).toContain('.image-modal.metadata-open .modal-meta{display:block');
    expect(styles).toContain('.modal-meta{display:none');
    expect(styles).toContain('max-height:56vh');
  });

  test('mobile modal toolbar is centered above the safe-area bottom', () => {
    const styles = css();
    expect(styles).toContain('.modalbar{left:50%');
    expect(styles).toContain('transform:translateX(-50%)');
    expect(styles).toContain('justify-content:center');
    expect(styles).toContain('bottom:calc(env(safe-area-inset-bottom, 0px) + 72px)');
  });

  test('app confirmation dialogs render above the mobile image preview overlay', () => {
    const styles = css();
    const dialogZ = Number(styles.match(/\.dialog-backdrop\{[^}]*z-index:(\d+)/)?.[1]);
    const mobileModalZ = Number(styles.match(/\.image-modal\{z-index:(\d+)\}/)?.[1]);
    const mobileModalBarZ = Number(styles.match(/\.image-modal \.modalbar\{z-index:(\d+)\}/)?.[1]);

    expect(Number.isFinite(dialogZ)).toBe(true);
    expect(Number.isFinite(mobileModalZ)).toBe(true);
    expect(Number.isFinite(mobileModalBarZ)).toBe(true);
    expect(dialogZ).toBeGreaterThan(mobileModalZ);
    expect(dialogZ).toBeGreaterThan(mobileModalBarZ);
  });
});
