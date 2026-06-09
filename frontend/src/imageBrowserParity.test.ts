import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('image browser parity with standalone Hermes image browser', () => {
  test('grid thumbnails show the complete image and use the standalone column sizing', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('src={item.image_url}');
    expect(styles).toContain('object-fit:contain');
    expect(styles).not.toContain('object-fit:cover');
    expect(styles).toContain('grid-template-columns:repeat(auto-fill,minmax(260px,1fr))');
  });

  test('pagination is driven by scroll/sentinel instead of a load more button', () => {
    const source = app();
    expect(source).toContain('sentinelRef');
    expect(source).toContain('IntersectionObserver');
    expect(source).toContain('Scroll to load more');
    expect(source).not.toContain('Load more');
    expect(source).not.toContain('className="load-more"');
  });

  test('lazy loading uses standalone dynamic viewport page sizes instead of fixed 120', () => {
    const source = app();
    expect(source).toContain('const MAX_PAGE_SIZE = 120;');
    expect(source).toContain('const MIN_PRELOAD_DISTANCE_PX = 1800;');
    expect(source).toContain('initialPageSizeForViewport');
    expect(source).toContain('lazyPageSizeForViewport');
    expect(source).toContain('pageSizeForViewport(offset)');
    expect(source).toContain('const pageSize = pageSizeForViewport(offset);');
    expect(source).toContain('limit=${pageSize}');
    expect(source).toContain('const more = chunk.length === pageSize;');
    expect(source).not.toContain('const PAGE_SIZE = 120;');
    expect(source).not.toContain('limit=${PAGE_SIZE}');
  });

  test('top summary formats bytes as KB MB GB TB and action buttons match selection mode', () => {
    const source = app();
    expect(source).toContain("['B', 'KB', 'MB', 'GB', 'TB']");
    expect(source).toContain('selecting, setSelecting');
    expect(source).toContain('Download selected');
    expect(source).toContain('Organize');
    expect(source).toContain('Delete selected');
    expect(source).toContain('selected.size > 0');
    expect(source).not.toContain('ZIP {selected.size');
  });

  test('cards show only images until hover, and checkboxes only exist in selection mode', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('image-overlay');
    expect(source).toContain('image-checkbox');
    expect(source).toContain('{selecting &&');
    expect(source).not.toContain('> select</label>');
    expect(source).not.toContain('image-card-meta');
    expect(styles).toContain('.image-card:hover .image-overlay');
    expect(styles).toContain('.image-checkbox');
  });

  test('modal uses full preview image, navigation buttons, and orientation-aware metadata layout', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('modalImageUrl');
    expect(source).toContain('item.png_url || item.image_url');
    expect(source).toContain('navigateModal(-1)');
    expect(source).toContain('navigateModal(1)');
    expect(source).toContain('metadataPlacement');
    expect(source).toContain('metadata-bottom');
    expect(styles).toContain('max-width:min(calc(100vw - 44px),1500px)');
    expect(styles).toContain('max-height:calc(100vh - 126px)');
    expect(styles).toContain('.image-modal.metadata-bottom');
    expect(styles).toContain('.modal-meta.metadata-bottom');
  });
});
