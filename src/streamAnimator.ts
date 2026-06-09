export const STREAM_FRAME_MS = 28;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

export function streamChunkSize(backlog: number, completing: boolean) {
  if (backlog <= 0) return 0;
  if (backlog <= 2) return 1;
  const targetFrames = completing ? 32 : 42;
  const minChunk = completing ? 3 : 1;
  const maxChunk = completing ? 36 : 24;
  return clamp(Math.ceil(backlog / targetFrames), minChunk, maxChunk);
}

type TimerHandle = ReturnType<typeof setTimeout> | number;
type StreamAnimatorOptions = {
  onUpdate: (text: string) => void;
  schedule?: (callback: () => void, delayMs: number) => TimerHandle;
  cancel?: (handle: TimerHandle) => void;
};

export function createStreamAnimator(options: StreamAnimatorOptions) {
  const schedule = options.schedule || ((callback, delayMs) => window.setTimeout(callback, delayMs));
  const cancelTimer = options.cancel || ((handle) => window.clearTimeout(handle as number));
  let target = '';
  let displayed = '';
  let timer: TimerHandle | null = null;
  let completing = false;
  let cancelled = false;
  let finishResolver: (() => void) | null = null;

  const resolveIfFinished = () => {
    if (completing && displayed.length >= target.length && finishResolver) {
      const resolve = finishResolver;
      finishResolver = null;
      resolve();
    }
  };

  const scheduleTick = () => {
    if (cancelled || timer !== null || displayed.length >= target.length) return;
    timer = schedule(tick, STREAM_FRAME_MS);
  };

  const tick = () => {
    timer = null;
    if (cancelled) return;
    const backlog = target.length - displayed.length;
    if (backlog <= 0) {
      resolveIfFinished();
      return;
    }
    const nextLength = displayed.length + streamChunkSize(backlog, completing);
    displayed = target.slice(0, nextLength);
    options.onUpdate(displayed);
    if (displayed.length < target.length) scheduleTick();
    else resolveIfFinished();
  };

  const setTarget = (text: string) => {
    target = text;
    if (!target.startsWith(displayed)) displayed = '';
    scheduleTick();
  };

  return {
    append(delta: string) {
      if (!delta || cancelled) return;
      target += delta;
      scheduleTick();
    },
    setTarget,
    finish(finalText?: string) {
      if (finalText !== undefined) setTarget(finalText);
      completing = true;
      if (displayed.length >= target.length) return Promise.resolve();
      return new Promise<void>((resolve) => {
        finishResolver = resolve;
        scheduleTick();
      });
    },
    cancel() {
      cancelled = true;
      if (timer !== null) {
        cancelTimer(timer);
        timer = null;
      }
      if (finishResolver) {
        const resolve = finishResolver;
        finishResolver = null;
        resolve();
      }
    },
    getDisplayed: () => displayed,
    getTarget: () => target,
  };
}
