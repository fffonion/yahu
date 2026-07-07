import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('image browser download and refresh parity', () => {
  test('selected downloads use browser multi-file downloads instead of zip batch API', () => {
    const source = app();
    expect(source).toContain('const downloadSelectedFiles = async (names: string[]) =>');
    expect(source).toContain('for (const item of selectedItems)');
    expect(source).toContain('triggerBrowserDownload(item.download_url || item.png_url, item.download_filename || item.filename)');
    expect(source).toContain('await new Promise((resolve) => window.setTimeout(resolve, 120));');
    expect(source).not.toContain("fetch('/image-api/batch-download'");
    expect(source).not.toContain('hermes-images.zip');
    expect(source).not.toContain('batch-download');
  });

  test('HEIC download labels are generic download labels and image browser button text is smaller', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("const downloadButtonLabel = (item: ImageEntry) => item.heic_status === 'missing' ? t('gallery.generateHeic') : t('gallery.download');");
    expect(source).toContain('aria-label={downloadButtonLabel(item)}');
    expect(source).toContain('aria-label={downloadButtonLabel(modal)}');
    expect(source).not.toContain("item.download_label}</span>");
    expect(source).not.toContain("modal.download_label}</span>");
    expect(styles).toMatch(/\.image-actions button,\.image-overlay button,\.modalbar button\{[^}]*font-size:12px/);
    expect(styles).toMatch(/\.image-name\{[^}]*font-size:11px/);
  });

  test('manual refresh and SSE resync follow standalone incremental refresh behavior', () => {
    const source = app();
    expect(source).toContain('const reloadQueuedRef = useRef(false);');
    expect(source).toContain('const refreshBusyRef = useRef(false);');
    expect(source).toContain('const refreshIncremental = useCallback(async () =>');
    expect(source).toContain("new URLSearchParams({ after: String(after), limit: String(MAX_PAGE_SIZE) })");
    expect(source).toContain("filter((item) => !item.heic_url).slice(0, 240).map((item) => item.filename)");
    expect(source).toContain("fetch(`/image-api/images/refresh?${params.toString()}`");
    expect(source).toContain('for (const item of [...payload.new_items || [], ...payload.checked_items || []])');
    expect(source).toContain("if (msg.type === 'resync') refresh()");
    expect(source).toContain('if (reloadQueuedRef.current) {');
  });
});
