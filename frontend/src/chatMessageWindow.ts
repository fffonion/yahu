export type MessageWindowDirection = 'latest' | 'older' | 'newer';

export type MessageWindowMergeInput<T> = {
  current: T[];
  chunk: T[];
  direction: MessageWindowDirection;
  limit: number;
  hasOlder: boolean;
  hasNewer: boolean;
  pageHasOlder: boolean;
  pageHasNewer: boolean;
};

export type MessageWindowMergeResult<T> = {
  messages: T[];
  hasOlder: boolean;
  hasNewer: boolean;
};

export function mergeMessageWindow<T>(input: MessageWindowMergeInput<T>): MessageWindowMergeResult<T> {
  const limit = Math.max(1, input.limit);
  if (input.direction === 'latest') {
    return {
      messages: input.chunk.slice(-limit),
      hasOlder: input.pageHasOlder,
      hasNewer: input.pageHasNewer,
    };
  }
  if (input.direction === 'older') {
    const merged = [...input.chunk, ...input.current];
    return {
      messages: merged.slice(0, limit),
      hasOlder: input.pageHasOlder,
      hasNewer: input.hasNewer || merged.length > limit,
    };
  }
  const merged = [...input.current, ...input.chunk];
  return {
    messages: merged.slice(-limit),
    hasOlder: input.hasOlder || merged.length > limit,
    hasNewer: input.pageHasNewer,
  };
}
