// Snapshot the Insights chart card into a PNG without a canvas library and
// without foreignObject HTML: Chromium taints the canvas when an SVG image
// carries HTML inside a foreignObject, so `toBlob` fails. Instead the card is rebuilt
// as one flat SVG — the plot SVG is cloned with its computed styles inlined,
// and the HTML-only parts (axes, legend, model-share bar) are redrawn as SVG
// text and rects at their measured positions. Every coordinate comes from
// getBoundingClientRect relative to the card, so the result covers the whole
// element even when it is scrolled out of view or taller than the viewport.

export const SNAPSHOT_STYLE_PROPS = [
  'display', 'position', 'top', 'left', 'right', 'bottom', 'width', 'height',
  'margin-top', 'margin-right', 'margin-bottom', 'margin-left',
  'padding-top', 'padding-right', 'padding-bottom', 'padding-left',
  'box-sizing', 'border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width',
  'border-style', 'border-color', 'border-radius',
  'background-color', 'background-image',
  'color', 'font-family', 'font-size', 'font-weight', 'font-style', 'line-height',
  'letter-spacing', 'text-align', 'white-space', 'text-overflow',
  'overflow', 'opacity', 'visibility',
  'transform', 'transform-origin', 'filter',
  'flex-direction', 'align-items', 'justify-content', 'gap',
  'grid-template-columns', 'grid-template-rows',
  'z-index',
  'fill', 'stroke', 'stroke-width', 'stroke-dasharray', 'stroke-linecap', 'stroke-linejoin',
  'stop-color', 'stop-opacity', 'fill-opacity', 'stroke-opacity', 'paint-order',
] as const;

/** Subtrees that only exist for pointer interaction and must not be drawn. */
export const SNAPSHOT_HIDDEN_SELECTORS = ['.chart-points', '.chart-stack-toggle', '.insights-snapshot'] as const;

export function snapshotFileName(metric: string, period: number): string {
  const safe = String(metric || '').replace(/[^a-z0-9_-]+/gi, '');
  return `yahu-insights-${period}d-${safe || 'usage'}.png`;
}

export type SnapshotDelivery = 'share' | 'download' | 'none';

export function decideSnapshotDelivery(input: { canShareFiles: boolean; canDownload: boolean }): SnapshotDelivery {
  if (input.canShareFiles) return 'share';
  if (input.canDownload) return 'download';
  return 'none';
}

export type SnapshotSize = { width: number; height: number };

export function planSnapshotPixels(size: SnapshotSize, scale: number): SnapshotSize {
  const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return {
    width: Math.max(1, Math.round(size.width * safeScale)),
    height: Math.max(1, Math.round(size.height * safeScale)),
  };
}

export function buildChartSnapshotSvgDocument(width: number, height: number, body: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">${body}</svg>`;
}

function escapeXml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function inlineStyles(source: Element, target: Element): void {
  const computed = getComputedStyle(source);
  let css = '';
  for (const prop of SNAPSHOT_STYLE_PROPS) {
    const value = computed.getPropertyValue(prop);
    if (value) css += `${prop}:${value};`;
  }
  target.setAttribute('style', css);
  if (source instanceof SVGSVGElement && target instanceof SVGSVGElement) {
    // Keep the plot's own coordinate space; width/height are set by the caller.
    target.removeAttribute('viewBox');
    const viewBox = source.getAttribute('viewBox');
    if (viewBox) target.setAttribute('viewBox', viewBox);
  }
  const from = source.children;
  const to = target.children;
  for (let index = 0; index < from.length && index < to.length; index += 1) {
    inlineStyles(from[index], to[index]);
  }
}

type Box = { left: number; top: number; width: number; height: number };

function relativeBox(element: Element, origin: DOMRect): Box {
  const rect = element.getBoundingClientRect();
  return { left: rect.left - origin.left, top: rect.top - origin.top, width: rect.width, height: rect.height };
}

/**
 * Rebuild the chart card as one SVG document. Coordinates are measured against
 * the card's own box, never the viewport, so nothing is clipped by scrolling.
 */
export function buildChartSnapshotSvg(card: HTMLElement, background: string): { svg: string; width: number; height: number } {
  const origin = card.getBoundingClientRect();
  const width = Math.max(1, Math.ceil(origin.width));
  const height = Math.max(1, Math.ceil(card.scrollHeight || origin.height));
  const body: string[] = [`<rect x="0" y="0" width="${width}" height="${height}" rx="16" fill="${escapeXml(background)}"/>`];

  for (const node of card.querySelectorAll('svg')) {
    const clone = node.cloneNode(true) as SVGSVGElement;
    inlineStyles(node, clone);
    for (const selector of SNAPSHOT_HIDDEN_SELECTORS) {
      clone.querySelectorAll<SVGElement>(selector).forEach((hidden) => {
        hidden.setAttribute('visibility', 'hidden');
      });
    }
    const box = relativeBox(node, origin);
    clone.setAttribute('x', String(Math.round(box.left)));
    clone.setAttribute('y', String(Math.round(box.top)));
    clone.setAttribute('width', String(Math.max(1, Math.round(box.width))));
    clone.setAttribute('height', String(Math.max(1, Math.round(box.height))));
    clone.setAttribute('preserveAspectRatio', 'none');
    body.push(new XMLSerializer().serializeToString(clone));
  }

  const drawText = (element: Element, anchor: 'start' | 'middle' | 'end' = 'start') => {
    const value = (element.textContent || '').trim();
    if (!value) return;
    const computed = getComputedStyle(element);
    const box = relativeBox(element, origin);
    const fontSize = Number.parseFloat(computed.fontSize) || 12;
    const x = anchor === 'middle' ? box.left + box.width / 2 : anchor === 'end' ? box.left + box.width : box.left;
    const y = box.top + box.height / 2 + fontSize * 0.36;
    body.push(
      `<text x="${x}" y="${y}" fill="${escapeXml(computed.color)}" font-size="${fontSize}" font-family="${escapeXml(computed.fontFamily)}" font-weight="${escapeXml(computed.fontWeight)}" text-anchor="${anchor}">${escapeXml(value)}</text>`,
    );
  };

  const drawSwatch = (element: Element) => {
    const computed = getComputedStyle(element);
    const box = relativeBox(element, origin);
    if (!box.width || !box.height) return;
    body.push(
      `<rect x="${box.left}" y="${box.top}" width="${box.width}" height="${box.height}" rx="${Math.min(box.width, box.height) / 2}" fill="${escapeXml(computed.backgroundColor)}"/>`,
    );
  };

  card.querySelectorAll('.chart-y-axis span').forEach((label) => drawText(label, 'end'));
  card.querySelectorAll('.chart-axis span').forEach((label) => drawText(label, 'middle'));
  card.querySelectorAll('.chart-legend span').forEach((item) => {
    const swatch = item.querySelector('i');
    if (swatch) drawSwatch(swatch);
    const textNode = item.cloneNode(true) as Element;
    textNode.querySelector('i')?.remove();
    drawText(textNode, 'start');
  });

  card.querySelectorAll('.usage-share-bar').forEach((bar) => {
    const box = relativeBox(bar, origin);
    body.push(`<rect x="${box.left}" y="${box.top}" width="${box.width}" height="${box.height}" rx="${box.height / 2}" fill="none" stroke="rgba(255,255,255,0.14)"/>`);
  });
  card.querySelectorAll('.usage-share-segment').forEach((segment) => {
    const computed = getComputedStyle(segment);
    const box = relativeBox(segment, origin);
    if (!box.width || !box.height) return;
    body.push(`<rect x="${box.left}" y="${box.top}" width="${box.width}" height="${box.height}" fill="${escapeXml(computed.backgroundColor)}"/>`);
  });
  card.querySelectorAll('.usage-share-indicator').forEach((indicator) => {
    const dot = indicator.querySelector('i');
    if (dot) drawSwatch(dot);
    const label = indicator.querySelector('b');
    if (label) drawText(label, 'start');
    const percent = indicator.querySelector('em');
    if (percent) drawText(percent, 'end');
  });

  return { svg: buildChartSnapshotSvgDocument(width, height, body.join('')), width, height };
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('snapshot image failed to load'));
    image.src = url;
  });
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error('snapshot canvas produced no blob'));
    }, 'image/png');
  });
}

export type CaptureOptions = { scale?: number; background?: string };

export async function captureElementPng(element: HTMLElement, options: CaptureOptions = {}): Promise<Blob> {
  const scale = options.scale ?? 2;
  const background = options.background ?? '#0b0d12';
  const { svg, width, height } = buildChartSnapshotSvg(element, background);
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml;charset=utf-8' }));
  try {
    const image = await loadImage(url);
    const pixels = planSnapshotPixels({ width, height }, scale);
    const canvas = document.createElement('canvas');
    canvas.width = pixels.width;
    canvas.height = pixels.height;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('snapshot canvas unavailable');
    context.fillStyle = background;
    context.fillRect(0, 0, pixels.width, pixels.height);
    context.drawImage(image, 0, 0, pixels.width, pixels.height);
    return await canvasToBlob(canvas);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Share the PNG through the native share sheet when possible, else download it. */
export async function deliverSnapshot(blob: Blob, filename: string): Promise<SnapshotDelivery> {
  const file = new File([blob], filename, { type: 'image/png' });
  const nav = navigator as Navigator & { canShare?: (data: ShareData) => boolean; share?: (data: ShareData) => Promise<void> };
  const canShareFiles = typeof nav.canShare === 'function' && typeof nav.share === 'function' && nav.canShare({ files: [file] });
  const canDownload = typeof document.createElement === 'function';
  const delivery = decideSnapshotDelivery({ canShareFiles, canDownload });
  if (delivery === 'share' && nav.share) {
    try {
      await nav.share({ files: [file] });
      return 'share';
    } catch {
      // The user dismissed the sheet; fall through to a plain download.
    }
  }
  if (!canDownload) return 'none';
  const href = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = href;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(href), 4000);
  return 'download';
}
