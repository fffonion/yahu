import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('image browser mobile modal touch parity', () => {
  test('modal implements standalone-style pinch zoom and pan state', () => {
    const source = app();
    expect(source).toContain('MODAL_MIN_ZOOM');
    expect(source).toContain('MODAL_MAX_ZOOM');
    expect(source).toContain('modalZoom');
    expect(source).toContain('modalPointers');
    expect(source).toContain('modalPinchStart');
    expect(source).toContain('zoomModalAt');
    expect(source).toContain('applyModalZoom');
    expect(source).toContain('scale(${modalZoom.current.scale})');
    expect(source).toContain("modalPointers.current.size >= 2");
  });

  test('modal touch swipes follow the finger and navigate or close on commit', () => {
    const source = app();
    expect(source).toContain('modalSwipeStart');
    expect(source).toContain('modalTransform');
    expect(source).toContain('translate3d');
    expect(source).toContain('snapModalImageBack');
    expect(source).toContain('animateModalClose');
    expect(source).toContain('navigateModal(amount < 0 ? 1 : -1');
    expect(source).toContain('Math.abs(amount) > 60');
    expect(source).toContain('suppressModalClickBriefly');
  });

  test('mobile modal supports tap-to-close while desktop clicks stay open', () => {
    const source = app();
    expect(source).toContain('isMobilePreviewMode');
    expect(source).toContain("window.matchMedia('(hover: none), (pointer: coarse)')");
    expect(source).toContain('if (!isMobilePreviewMode()) return');
    expect(source).toContain('setModal(null)');
  });

  test('mobile modal CSS disables browser gesture interference and hides keyboard hints', () => {
    const styles = css();
    expect(styles).toContain('.image-modal{touch-action:none');
    expect(styles).toContain('.image-modal-img{touch-action:none');
    expect(styles).toContain('.desktop-key-hint{display:none}');
    expect(styles).toContain('.image-modal-img.zoomed');
    expect(styles).toContain('.image-modal-img.panning');
  });
});
