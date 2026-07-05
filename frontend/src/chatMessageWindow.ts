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

function messageKey(message: unknown): string | null {
  if (!message || typeof message !== 'object') return null;
  const id = (message as { id?: unknown }).id;
  if (id === null || id === undefined) return null;
  return String(id);
}

function uniqueMessages<T>(messages: T[]): T[] {
  const seen = new Set<string>();
  const out: T[] = [];
  for (const message of messages) {
    const key = messageKey(message);
    if (key && seen.has(key)) continue;
    if (key) seen.add(key);
    out.push(message);
  }
  return out;
}

function newMessagesOnly<T>(chunk: T[], current: T[]): T[] {
  const currentIds = new Set(current.map(messageKey).filter((key): key is string => Boolean(key)));
  return chunk.filter((message) => {
    const key = messageKey(message);
    return !key || !currentIds.has(key);
  });
}

export function mergeMessageWindow<T>(input: MessageWindowMergeInput<T>): MessageWindowMergeResult<T> {
  const limit = Math.max(1, input.limit);
  if (input.direction === 'latest') {
    return {
      messages: uniqueMessages(input.chunk).slice(-limit),
      hasOlder: input.pageHasOlder,
      hasNewer: input.pageHasNewer,
    };
  }
  if (input.direction === 'older') {
    const olderChunk = newMessagesOnly(input.chunk, input.current);
    const merged = uniqueMessages([...olderChunk, ...input.current]);
    return {
      messages: merged.slice(0, limit),
      hasOlder: olderChunk.length > 0 ? input.pageHasOlder : false,
      hasNewer: input.hasNewer || merged.length > limit,
    };
  }
  const newerChunk = newMessagesOnly(input.chunk, input.current);
  const merged = uniqueMessages([...input.current, ...newerChunk]);
  return {
    messages: merged.slice(-limit),
    hasOlder: input.hasOlder || merged.length > limit,
    hasNewer: newerChunk.length > 0 ? input.pageHasNewer : false,
  };
}
