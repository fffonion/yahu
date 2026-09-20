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
  const atTop = isNearOlderBoundary(metrics);
  const cannotMoveScrollTop = metrics.scrollHeight <= metrics.clientHeight + 1;
  return atTop || cannotMoveScrollTop;
}

export function shouldAutoLoadOlderForHiddenHistory(metrics: ChatScrollMetrics, hasOlder: boolean, loading: boolean): boolean {
  return hasOlder && !loading && metrics.scrollHeight <= metrics.clientHeight + 1;
}

export type StreamFollowMode = 'auto' | 'follow' | 'away';
export type StreamFollowIntent = Exclude<StreamFollowMode, 'auto'> | null;
export type SessionStreamFollowState = { sessionId: string; mode: StreamFollowMode };
export type ProgrammaticChatScroll = { sessionId: string; scrollTop: number; generation: number; token: number };

export function sessionStreamFollowMode(state: SessionStreamFollowState, sessionId: string): StreamFollowMode {
  return state.sessionId === sessionId ? state.mode : 'auto';
}

export function matchesProgrammaticChatScroll(target: ProgrammaticChatScroll | null, sessionId: string, scrollTop: number, tolerancePx = 0.01): boolean {
  return !!target && target.sessionId === sessionId && Math.abs(target.scrollTop - scrollTop) <= tolerancePx;
}

export function shouldSyncMinimapToLatest(mode: StreamFollowMode, metrics: ChatScrollMetrics, autoThresholdPx = 220): boolean {
  if (mode === 'away') return false;
  if (mode === 'follow') return true;
  return isNearNewerBoundary(metrics, autoThresholdPx);
}

export function streamFollowIntentAfterScroll(previousScrollTop: number | null, metrics: ChatScrollMetrics, latestThresholdPx = 0): StreamFollowIntent {
  if (previousScrollTop !== null && metrics.scrollTop < previousScrollTop) return 'away';
  const distanceFromLatest = metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight;
  if (previousScrollTop !== null && metrics.scrollTop > previousScrollTop && distanceFromLatest <= latestThresholdPx) return 'follow';
  return null;
}
