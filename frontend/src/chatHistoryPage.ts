export type ChatHistoryRole = 'user' | 'assistant' | 'system' | 'tool';

export type RawChatHistoryMessage = {
  id?: string | number | null;
  role?: string | null;
  [key: string]: unknown;
};

export type ChatHistoryMessageLike = {
  id?: string;
  role: ChatHistoryRole | string;
};

export type ChatHistoryPageRaw = {
  data?: RawChatHistoryMessage[] | null;
  total?: number;
  has_older?: boolean;
  has_newer?: boolean;
  started_at?: number | string;
  last_active?: number | string;
  [key: string]: unknown;
};

const RENDERABLE_HISTORY_ROLES = new Set(['user', 'assistant', 'tool', 'system']);
const HISTORY_BOUNDARY_ROLES = new Set(['user', 'system']);

export function normalizeChatHistoryChunk<T extends ChatHistoryMessageLike>(items: RawChatHistoryMessage[] | null | undefined, normalize: (raw: RawChatHistoryMessage) => T): T[] {
  return (items || [])
    .filter((message) => RENDERABLE_HISTORY_ROLES.has(String(message.role || '')))
    .map(normalize);
}

export async function backfillOlderChunkToTurnBoundary<T extends ChatHistoryMessageLike>({
  firstPage,
  firstChunk,
  fetchBefore,
  normalizeChunk,
  numericId,
  pageLimit,
  rawWindowLimit,
  maxPages = 16,
}: {
  firstPage: ChatHistoryPageRaw;
  firstChunk: T[];
  fetchBefore: (before: string, limit: number) => Promise<ChatHistoryPageRaw>;
  normalizeChunk: (items: RawChatHistoryMessage[] | null | undefined) => T[];
  numericId: (id?: string) => string;
  pageLimit: number;
  rawWindowLimit: number;
  maxPages?: number;
}): Promise<{ chunk: T[]; pageHasOlder: boolean; pageHasNewer: boolean; boundaryPage: ChatHistoryPageRaw }> {
  let chunk = firstChunk;
  let pageHasOlder = Boolean(firstPage.has_older);
  const pageHasNewer = Boolean(firstPage.has_newer);
  let boundaryPage = firstPage;

  const seenBefore = new Set<string>();
  const seenMessageIds = new Set(chunk.map((message) => String(message.id || '')).filter(Boolean));

  for (let guard = 0; guard < maxPages && pageHasOlder && chunk.length > 0 && chunk.length < rawWindowLimit; guard += 1) {
    if (HISTORY_BOUNDARY_ROLES.has(String(chunk[0]?.role || ''))) break;
    const before = numericId(chunk[0]?.id);
    if (!before || seenBefore.has(before)) {
      pageHasOlder = false;
      break;
    }
    seenBefore.add(before);
    const olderPage = await fetchBefore(before, pageLimit);
    const olderChunk = normalizeChunk(olderPage.data || []);
    pageHasOlder = Boolean(olderPage.has_older);
    const newOlderChunk = olderChunk.filter((message) => {
      const id = String(message.id || '').trim();
      if (!id) return true;
      if (seenMessageIds.has(id)) return false;
      seenMessageIds.add(id);
      return true;
    });
    if (!newOlderChunk.length) {
      pageHasOlder = false;
      break;
    }
    chunk = [...newOlderChunk, ...chunk];
    boundaryPage = { ...olderPage, total: firstPage.total, last_active: firstPage.last_active };
  }

  return { chunk, pageHasOlder, pageHasNewer, boundaryPage };
}
