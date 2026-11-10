import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  buildSnapshotSvgDocument,
  decideSnapshotDelivery,
  planSnapshotPixels,
  SNAPSHOT_HIDDEN_SELECTORS,
  SNAPSHOT_STYLE_PROPS,
  snapshotFileName,
} from './chartSnapshot';

const module_ = () => readFileSync(new URL('./chartSnapshot.ts', import.meta.url), 'utf8');

describe('insights content snapshot', () => {
  test('file name carries the metric and period', () => {
    expect(snapshotFileName('cost_usd', 7)).toBe('yahu-insights-7d-cost_usd.png');
    expect(snapshotFileName('total_tokens', 30)).toBe('yahu-insights-30d-total_tokens.png');
    expect(snapshotFileName('', 1)).toBe('yahu-insights-1d-usage.png');
  });

  test('svg document is sized from the element box, not the viewport', () => {
    const document_ = buildSnapshotSvgDocument(1200, 1810, '<div xmlns="http://www.w3.org/1999/xhtml" style="width:1200px;height:1810px"></div>');
    expect(document_).toContain('<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1810" viewBox="0 0 1200 1810">');
    expect(document_).toContain('width:1200px;height:1810px');
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

  test('the style pass copies layout plus paint so every card survives', () => {
    const props: readonly string[] = SNAPSHOT_STYLE_PROPS;
    for (const prop of [
      'display', 'position', 'top', 'left', 'width', 'height', 'padding-top',
      'font-family', 'font-size', 'line-height', 'letter-spacing',
      'grid-template-columns', 'gap', 'flex-direction', 'border-radius',
      'background-color', 'color', 'opacity', 'z-index',
      'fill', 'stroke', 'stroke-width', 'text-anchor',
    ]) {
      expect(props).toContain(prop);
    }
  });

  test('interactive-only subtrees are excluded from the image', () => {
    expect(SNAPSHOT_HIDDEN_SELECTORS).toContain('.chart-points');
    expect(SNAPSHOT_HIDDEN_SELECTORS).toContain('.chart-stack-toggle');
  });

  test('the whole subtree is cloned with inlined styles into one foreignObject', () => {
    const source = module_();
    expect(source).toContain('inlineSnapshotStyles');
    expect(source).toContain('new XMLSerializer().serializeToString(clone)');
    expect(source).toContain('<foreignObject');
    expect(source).toContain('element.scrollWidth');
    expect(source).toContain('element.scrollHeight');
    expect(source).not.toContain("querySelectorAll('.chart-y-axis span')");
    expect(source).not.toContain("querySelectorAll('.usage-share-segment')");
  });

  test('oversized canvases are capped below the Chromium limit', () => {
    const source = module_();
    expect(source).toContain('maxDimension');
    expect(source).toContain('MAX_CANVAS_DIMENSION = 16384');
    expect(source).toContain('maxDimension / longest');
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

  test('the button captures the whole content area and shares or downloads it', () => {
    const source = app();
    expect(source).toContain("document.querySelector<HTMLElement>('.insights-content')");
    expect(source).toContain('await captureElementPng(panel, { scale: 2, background })');
    expect(source).toContain('await deliverSnapshot(blob, snapshotFileName(props.metric, props.period))');
  });
});
