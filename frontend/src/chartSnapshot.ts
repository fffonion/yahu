// Snapshot the Insights content area (everything below the header, right of the
// rail) into a PNG. The section is cloned, every visible element gets its
// computed styles inlined (an SVG data URL never loads the page's stylesheet),
// and the clone is serialized into one XML document that is rendered through a
// single foreignObject. Sizes come from the element's own scroll box, so content
// taller than or scrolled out of the viewport is still captured whole.

export const SNAPSHOT_STYLE_PROPS = [
  'display', 'position', 'top', 'left', 'right', 'bottom', 'width', 'height',
  'min-width', 'max-width', 'min-height', 'max-height', 'box-sizing',
  'margin-top', 'margin-right', 'margin-bottom', 'margin-left',
  'padding-top', 'padding-right', 'padding-bottom', 'padding-left',
  'border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width',
  'border-top-style', 'border-right-style', 'border-bottom-style', 'border-left-style',
  'border-color', 'border-radius',
  'background-color', 'background-image', 'background-size', 'background-position', 'background-repeat',
  'color', 'font-family', 'font-size', 'font-weight', 'font-style', 'font-variant',
  'line-height', 'letter-spacing', 'word-spacing',
  'text-align', 'text-transform', 'text-decoration-line', 'white-space', 'word-break',
  'overflow-wrap', 'text-overflow', 'overflow-x', 'overflow-y',
  'opacity', 'visibility', 'z-index',
  'flex-direction', 'flex-wrap', 'flex-grow', 'flex-shrink', 'flex-basis',
  'align-items', 'align-self', 'align-content', 'justify-content', 'justify-items', 'justify-self',
  'gap', 'row-gap', 'column-gap',
  'grid-template-columns', 'grid-template-rows', 'grid-auto-flow', 'grid-column', 'grid-row',
  'transform', 'transform-origin', 'filter', 'box-shadow',
  'fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-dasharray', 'stroke-linecap',
  'stroke-linejoin', 'stroke-opacity', 'stop-color', 'stop-opacity', 'paint-order',
  'text-anchor', 'dominant-baseline', 'vector-effect', 'shape-rendering',
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

export function buildSnapshotSvgDocument(width: number, height: number, body: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">${body}</svg>`;
}

function isHidden(source: Element): boolean {
  return SNAPSHOT_HIDDEN_SELECTORS.some((selector) => source.matches(selector));
}

function styleText(source: Element): string {
  const computed = getComputedStyle(source);
  let css = '';
  for (const prop of SNAPSHOT_STYLE_PROPS) {
    const value = computed.getPropertyValue(prop);
    if (!value) continue;
    // `fill: none` and `stroke: none` matter (SVG defaults would paint them
    // black); every other `none` is simply the initial value.
    if (value === 'none' && !(prop === 'fill' || prop === 'stroke' || prop === 'background-image')) continue;
    css += `${prop}:${value};`;
  }
  return css;
}

/**
 * Inline the computed paint and layout of every element in the subtree, so the
 * clone renders identically inside an isolated SVG document.
 */
export function inlineSnapshotStyles(source: Element, target: Element): void {
  if (isHidden(source)) {
    target.setAttribute('style', `${styleText(source)}display:none !important;`);
  } else {
    target.setAttribute('style', styleText(source));
  }
  if (source instanceof SVGSVGElement && target instanceof SVGSVGElement) {
    // Keep the plot's own coordinate space; width/height come from the inline style.
    const viewBox = source.getAttribute('viewBox');
    if (viewBox) target.setAttribute('viewBox', viewBox);
    target.removeAttribute('width');
    target.removeAttribute('height');
  }
  const from = source.children;
  const to = target.children;
  for (let index = 0; index < from.length && index < to.length; index += 1) {
    inlineSnapshotStyles(from[index], to[index]);
  }
}

/** Build one self-contained SVG document from the element and its own scroll box. */
export function buildSnapshotSvg(element: HTMLElement, background: string): { svg: string; width: number; height: number } {
  const width = Math.max(1, Math.ceil(Math.max(element.scrollWidth, element.clientWidth, element.getBoundingClientRect().width)));
  const height = Math.max(1, Math.ceil(Math.max(element.scrollHeight, element.clientHeight, element.getBoundingClientRect().height)));
  const clone = element.cloneNode(true) as HTMLElement;
  inlineSnapshotStyles(element, clone);
  clone.setAttribute('xmlns', 'http://www.w3.org/1999/xhtml');
  clone.style.width = `${width}px`;
  clone.style.height = `${height}px`;
  clone.style.margin = '0';
  clone.style.overflow = 'visible';
  clone.style.background = background;
  const markup = new XMLSerializer().serializeToString(clone);
  const body = `<foreignObject x="0" y="0" width="${width}" height="${height}">${markup}</foreignObject>`;
  return { svg: buildSnapshotSvgDocument(width, height, body), width, height };
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

export type CaptureOptions = { scale?: number; background?: string; maxDimension?: number };

/** Chromium refuses canvases whose width or height exceeds this. */
const MAX_CANVAS_DIMENSION = 16384;

export async function captureElementPng(element: HTMLElement, options: CaptureOptions = {}): Promise<Blob> {
  const requested = options.scale ?? 2;
  const background = options.background ?? '#0b0d12';
  const maxDimension = options.maxDimension ?? MAX_CANVAS_DIMENSION;
  const { svg, width, height } = buildSnapshotSvg(element, background);
  const longest = Math.max(width, height);
  const scale = longest * requested > maxDimension ? maxDimension / longest : requested;
  // A data URL keeps the SVG image same-origin with the page; a blob: URL makes
  // Chromium treat the foreignObject document as cross-origin and taints the
  // canvas, so `toBlob` would throw.
  const dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  const image = await loadImage(dataUrl);
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
