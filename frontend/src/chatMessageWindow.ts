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

function mergeLatestMessages<T>(chunk: T[], current: T[]): T[] {
  const currentById = new Map<string, T>();
  for (const message of current) {
    const key = messageKey(message);
    if (key) currentById.set(key, message);
  }
  const chunkIds = new Set<string>();
  const merged = chunk.map((message) => {
    const key = messageKey(message);
    if (!key) return message;
    chunkIds.add(key);
    return currentById.get(key) || message;
  });
  for (const message of current) {
    const key = messageKey(message);
    if (!key || !chunkIds.has(key)) merged.push(message);
  }
  return sortMessagesInDisplayOrder(uniqueMessages(merged));
}

export function sortMessagesInDisplayOrder<T>(messages: T[]): T[] {
  const indexed = messages.map((message, index) => ({ message, index }));
  const timestamps = indexed.map(({ message }) => {
    if (!message || typeof message !== 'object') return undefined;
    const value = (message as { timestamp?: unknown }).timestamp;
    const numeric = typeof value === 'number' ? value : Number(value);
    return Number.isFinite(numeric) ? numeric : undefined;
  });
  if (timestamps.every((value) => value !== undefined)) {
    return indexed
      .sort((left, right) => (timestamps[left.index]! - timestamps[right.index]!) || left.index - right.index)
      .map(({ message }) => message);
  }
  const ids = indexed.map(({ message }) => {
    const key = messageKey(message);
    if (!key || !/^\d+$/.test(key)) return undefined;
    return Number(key);
  });
  if (ids.every((value) => value !== undefined)) {
    return indexed
      .sort((left, right) => (ids[left.index]! - ids[right.index]!) || left.index - right.index)
      .map(({ message }) => message);
  }
  const partiallyIndexed = indexed.map(({ message, index }) => {
    const key = messageKey(message);
    const numericId = key && /^\d+$/.test(key) ? Number(key) : undefined;
    const timestamp = timestamps[index];
    return {
      message,
      index,
      value: numericId ?? timestamp,
    };
  });
  if (partiallyIndexed.some(({ value }) => value !== undefined)) {
    return partiallyIndexed
      .sort((left, right) => {
        if (left.value === undefined) return right.value === undefined ? left.index - right.index : 1;
        if (right.value === undefined) return -1;
        return left.value - right.value || left.index - right.index;
      })
      .map(({ message }) => message);
  }
  return messages;
}

export function mergeMessageWindow<T>(input: MessageWindowMergeInput<T>): MessageWindowMergeResult<T> {
  const limit = Math.max(1, input.limit);
  if (input.direction === 'latest') {
    const merged = mergeLatestMessages(input.chunk, input.current);
    return {
      messages: merged.slice(-limit),
      hasOlder: input.pageHasOlder || merged.length > limit,
      hasNewer: input.pageHasNewer,
    };
  }
  if (input.direction === 'older') {
    const olderChunk = newMessagesOnly(input.chunk, input.current);
    const merged = sortMessagesInDisplayOrder(uniqueMessages([...olderChunk, ...input.current]));
    return {
      messages: merged.slice(0, limit),
      hasOlder: olderChunk.length > 0 ? input.pageHasOlder : false,
      hasNewer: input.hasNewer || merged.length > limit,
    };
  }
  const newerChunk = newMessagesOnly(input.chunk, input.current);
  const merged = sortMessagesInDisplayOrder(uniqueMessages([...input.current, ...newerChunk]));
  return {
    messages: merged.slice(-limit),
    hasOlder: input.hasOlder || merged.length > limit,
    hasNewer: newerChunk.length > 0 ? input.pageHasNewer : false,
  };
}
