import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('image browser download and refresh parity', () => {
  test('selected downloads generate missing HEIC first, then trigger browser downloads in original mtime order with progress', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const downloadSelectedFiles = async (names: string[]) =>');
    expect(source).toContain(".sort((a, b) => b.modified_at - a.modified_at || b.filename.localeCompare(a.filename))");
    expect(source).toContain('await Promise.all(selectedItems.map(async (item, index) => {');
    expect(source).toContain('const preparedItems = new Array<ImageEntry | null>(selectedItems.length);');
    expect(source).toContain('const readyItems = preparedItems as ImageEntry[];');
    expect(source).toContain("for (let index = 0; index < readyItems.length; index += 1)");
    expect(source).toContain('generateHeic(item, false)');
    expect(source).toContain('const prepared = item.heic_status === \'missing\' ? await generateHeic(item, false) : item;');
    expect(source).toContain('triggerBrowserDownload(downloadItem.download_url || downloadItem.png_url');
    expect(source).toContain("setDownloadProgress({ current: preparedCount, total: selectedItems.length, filename: item.filename, phase: 'preparing' });");
    expect(source).toContain('className="gallery-download-progress"');
    expect(source).toContain('gallery-download-progress-track');
    expect(styles).toContain('.gallery-download-progress{position:fixed;right:20px;bottom:20px;');
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

  test('gallery toolbar actions are icon-only and selection state uses icons', () => {
    const source = app();
    expect(source).toContain('<Download aria-hidden="true" />');
    expect(source).toContain('<CalendarClock aria-hidden="true" />');
    expect(source).toContain('<Trash2 aria-hidden="true" />');
    expect(source).toContain("{selecting ? <X aria-hidden=\"true\" /> : <CheckSquare aria-hidden=\"true\" />}");
    expect(source).not.toContain('downloadSelected}</span>');
    expect(source).not.toContain('organize}</span>');
    expect(source).not.toContain('deleteSelected}</span>');
    expect(source).not.toContain('selecting ? t(\'gallery.cancel\') : t(\'gallery.select\')');
  });
  test('selected gallery batch actions use the shared icon button styling', () => {
    const source = app();
    expect(source).toContain('<button className="icon-btn mobile-icon-only" aria-label={t(\'gallery.downloadSelected\')}');
    expect(source).toContain('<button className="icon-btn mobile-icon-only" aria-label={t(\'gallery.organize\')}');
    expect(source).toContain('<button className="icon-btn danger mobile-icon-only" aria-label={t(\'gallery.deleteSelected\')}');
  });

  test('context menu never labels a PNG fallback as a HEIC download', () => {
    const source = app();
    expect(source).toContain("{imageMenu.item.heic_status !== 'not_applicable' && <button type=\"button\" role=\"menuitem\" onClick={() => { downloadOne(imageMenu.item); }}><Download /> {t('gallery.downloadHEIC')}</button>}");
    expect(source).not.toContain('triggerBrowserDownload(imageMenu.item.heic_url || imageMenu.item.png_url');
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
