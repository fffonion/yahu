export type ViewportHeightSource = {
  innerHeight: number;
  visualViewport?: { height: number } | null;
};

export function visibleViewportHeight(source: ViewportHeightSource) {
  const visualHeight = Number(source.visualViewport?.height);
  if (Number.isFinite(visualHeight) && visualHeight > 0) return Math.round(visualHeight);
  const layoutHeight = Number(source.innerHeight);
  return Math.max(1, Math.round(Number.isFinite(layoutHeight) ? layoutHeight : 1));
}
