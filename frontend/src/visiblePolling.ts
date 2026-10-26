type VisibilitySource = {
  readonly hidden: boolean;
  addEventListener(type: 'visibilitychange', listener: () => void): void;
  removeEventListener(type: 'visibilitychange', listener: () => void): void;
};
type PollClock = {
  setInterval(callback: () => void, interval: number): number;
  clearInterval(timer: number): void;
};

/** Keep foreground freshness without background timers or overlapping requests. */
export function startVisiblePolling(
  task: () => void | Promise<unknown>,
  intervalMs: number,
  immediate = false,
  source: VisibilitySource = document,
  clock: PollClock = window,
): () => void {
  let timer: number | null = null;
  let stopped = false;
  let pending = false;
  const run = () => {
    if (stopped || source.hidden || pending) return;
    pending = true;
    try {
      Promise.resolve(task()).catch(() => {}).finally(() => { pending = false; });
    } catch {
      pending = false;
    }
  };
  const pause = () => {
    if (timer !== null) clock.clearInterval(timer);
    timer = null;
  };
  const resume = () => {
    pause();
    if (stopped || source.hidden) return;
    timer = clock.setInterval(run, intervalMs);
    run();
  };
  const onVisibilityChange = () => { if (source.hidden) pause(); else resume(); };
  source.addEventListener('visibilitychange', onVisibilityChange);
  if (!source.hidden) {
    timer = clock.setInterval(run, intervalMs);
    if (immediate) run();
  }
  return () => {
    stopped = true;
    pause();
    source.removeEventListener('visibilitychange', onVisibilityChange);
  };
}
