import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  buildChartSnapshotSvgDocument,
  decideSnapshotDelivery,
  planSnapshotPixels,
  SNAPSHOT_HIDDEN_SELECTORS,
  SNAPSHOT_STYLE_PROPS,
  snapshotFileName,
} from './chartSnapshot';

const module_ = () => readFileSync(new URL('./chartSnapshot.ts', import.meta.url), 'utf8');

describe('insights chart snapshot', () => {
  test('file name carries the metric and period', () => {
    expect(snapshotFileName('cost_usd', 7)).toBe('yahu-insights-7d-cost_usd.png');
    expect(snapshotFileName('total_tokens', 30)).toBe('yahu-insights-30d-total_tokens.png');
    expect(snapshotFileName('', 1)).toBe('yahu-insights-1d-usage.png');
  });

  test('svg document is sized from the element box, not the viewport', () => {
    const document_ = buildChartSnapshotSvgDocument(1200, 640, '<rect width="1200" height="640" fill="#000"/>');
    expect(document_).toContain('<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="640" viewBox="0 0 1200 640">');
    expect(document_).toContain('<rect width="1200" height="640" fill="#000"/>');
  });

  test('pixel size scales from the element box', () => {
    expect(planSnapshotPixels({ width: 1120, height: 640 }, 2)).toEqual({ width: 2240, height: 1280 });
    expect(planSnapshotPixels({ width: 1120, height: 640 }, 0)).toEqual({ width: 1120, height: 640 });
  });

  test('native share wins when available, download is the fallback', () => {
    expect(decideSnapshotDelivery({ canShareFiles: true, canDownload: true })).toBe('share');
    expect(decideSnapshotDelivery({ canShareFiles: false, canDownload: true })).toBe('download');
    expect(decideSnapshotDelivery({ canShareFiles: false, canDownload: false })).toBe('none');
  });

  test('the style pass copies layout plus paint so axes and legend survive', () => {
    const props: readonly string[] = SNAPSHOT_STYLE_PROPS;
    for (const prop of ['position', 'top', 'left', 'width', 'height', 'font-family', 'fill', 'stroke', 'opacity', 'z-index']) {
      expect(props).toContain(prop);
    }
  });

  test('interactive-only subtrees are excluded from the image', () => {
    expect(SNAPSHOT_HIDDEN_SELECTORS).toContain('.chart-points');
    expect(SNAPSHOT_HIDDEN_SELECTORS).toContain('.chart-stack-toggle');
  });

  test('the rasterizer never emits a foreignObject element (Chromium taints the canvas)', () => {
    expect(module_()).not.toContain('<foreignObject');
  });

  test('HTML-only chart parts are redrawn as SVG text and rects', () => {
    const source = module_();
    expect(source).toContain("querySelectorAll('.chart-y-axis span')");
    expect(source).toContain("querySelectorAll('.chart-axis span')");
    expect(source).toContain("querySelectorAll('.chart-legend span')");
    expect(source).toContain("querySelectorAll('.usage-share-segment')");
    expect(source).toContain("querySelectorAll('.usage-share-indicator')");
    expect(source).toContain('text-anchor');
  });
});

describe('insights snapshot wiring', () => {
  const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

  test('a camera button sits in the insights header toolstrip', () => {
    const source = app();
    expect(source).toContain('import { captureElementPng, deliverSnapshot, snapshotFileName } from \'./chartSnapshot\';');
    expect(source).toContain('className="icon-btn insights-snapshot"');
    expect(source).toContain('aria-label={t(\'insights.snapshot\')}');
    const header = source.split('insights-header')[1] || '';
    expect(header).toContain('insights-snapshot');
  });

  test('the button captures only the chart card and shares or downloads it', () => {
    const source = app();
    expect(source).toContain("document.querySelector<HTMLElement>('.insights-chart-card')");
    expect(source).toContain('await captureElementPng(card, { scale: 2, background })');
    expect(source).toContain('await deliverSnapshot(blob, snapshotFileName(props.metric, props.period))');
  });
});
