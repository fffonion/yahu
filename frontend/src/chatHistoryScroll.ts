export type ChatScrollMetrics = Pick<HTMLElement, 'scrollTop' | 'scrollHeight' | 'clientHeight'>;

export function isNearOlderBoundary(metrics: ChatScrollMetrics, thresholdPx = 80): boolean {
  return metrics.scrollTop < thresholdPx;
}

export function isNearNewerBoundary(metrics: ChatScrollMetrics, thresholdPx = 80): boolean {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight < thresholdPx;
}

export function shouldLoadOlderFromScroll(metrics: ChatScrollMetrics, hasOlder: boolean, loading: boolean, thresholdPx = 80): boolean {
  return hasOlder && !loading && isNearOlderBoundary(metrics, thresholdPx);
}

export function shouldLoadNewerFromScroll(metrics: ChatScrollMetrics, hasNewer: boolean, loading: boolean, thresholdPx = 80): boolean {
  return hasNewer && !loading && isNearNewerBoundary(metrics, thresholdPx);
}

export function shouldLoadOlderFromWheel(metrics: ChatScrollMetrics, deltaY: number, hasOlder: boolean, loading: boolean): boolean {
  if (deltaY >= 0) return false;
  if (!hasOlder || loading) return false;
  const atTop = metrics.scrollTop <= 0;
  const cannotMoveScrollTop = metrics.scrollHeight <= metrics.clientHeight + 1;
  return atTop || cannotMoveScrollTop;
}
