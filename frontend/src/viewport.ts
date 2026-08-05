export type ViewportHeightSource = {
  innerHeight: number;
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

export function visibleViewportHeight(source: ViewportHeightSource) {
  const visualHeight = Number(source.visualViewport?.height);
  if (Number.isFinite(visualHeight) && visualHeight > 0) return Math.round(visualHeight);
  const layoutHeight = Number(source.innerHeight);
  return Math.max(1, Math.round(Number.isFinite(layoutHeight) ? layoutHeight : 1));
}
