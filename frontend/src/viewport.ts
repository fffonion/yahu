export type ViewportHeightSource = {
  innerHeight: number;
  clientHeight?: number;
  visualViewport?: { height: number } | null;
};

export type FocusedElementLike = {
  tagName?: string;
  isContentEditable?: boolean;
} | null;

export function isTextEntryElement(element: FocusedElementLike) {
  const tagName = String(element?.tagName || '').toUpperCase();
  return tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT' || element?.isContentEditable === true;
}

function layoutViewportHeight(source: ViewportHeightSource) {
  const heights = [source.innerHeight, source.clientHeight]
    .map(Number)
    .filter((height) => Number.isFinite(height) && height > 0);
  return Math.max(1, Math.round(heights.length > 0 ? Math.max(...heights) : 1));
}

export function visibleViewportHeight(source: ViewportHeightSource) {
  const visualHeight = Number(source.visualViewport?.height);
  if (Number.isFinite(visualHeight) && visualHeight > 0) return Math.round(visualHeight);
  return layoutViewportHeight(source);
}

export function resumedViewportHeight(source: ViewportHeightSource) {
  return Math.max(visibleViewportHeight(source), layoutViewportHeight(source));
}
