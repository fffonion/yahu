import { describe, expect, test } from 'bun:test';
import { backfillOlderChunkToTurnBoundary, normalizeChatHistoryChunk, numericHistoryMessageId, type ChatHistoryPageRaw } from './chatHistoryPage';

type Msg = { id: string; role: string; content?: string };
const normalize = (raw: any): Msg => ({ id: String(raw.id), role: String(raw.role), content: raw.content });
const numericId = numericHistoryMessageId;

describe('chat history page helpers', () => {
  test('accepts signed synthetic ids for stitched history cursors and jumps', () => {
    expect(numericHistoryMessageId('-8999999999395')).toBe('-8999999999395');
    expect(numericHistoryMessageId('291019')).toBe('291019');
    expect(numericHistoryMessageId('message-1')).toBe('');
  });

  test('normalizes only renderable chat roles while preserving order', () => {
    const chunk = normalizeChatHistoryChunk([
      { id: 1, role: 'user' },
      { id: 2, role: 'debug' },
      { id: 3, role: 'assistant' },
      { id: 4, role: 'tool' },
      { id: 5, role: 'system' },
    ], normalize);

    expect(chunk.map((message) => message.id)).toEqual(['1', '3', '4', '5']);
  });

  test('backfills older pages until a user or system boundary before merging', async () => {
    const calls: string[] = [];
    const pages: Record<string, ChatHistoryPageRaw> = {
      '30': { data: [{ id: 20, role: 'assistant' }, { id: 21, role: 'tool' }], has_older: true, total: 99, last_active: 'latest' },
      '20': { data: [{ id: 10, role: 'user' }, { id: 11, role: 'assistant' }], has_older: false, total: 12, last_active: 'old' },
    };
    const firstPage: ChatHistoryPageRaw = { data: [{ id: 30, role: 'tool' }, { id: 31, role: 'assistant' }], has_older: true, has_newer: false, total: 99, last_active: 'latest' };
    const result = await backfillOlderChunkToTurnBoundary({
      firstPage,
      firstChunk: normalizeChatHistoryChunk(firstPage.data, normalize),
      fetchBefore: async (before) => { calls.push(before); return pages[before]; },
      normalizeChunk: (items) => normalizeChatHistoryChunk(items, normalize),
      numericId,
      pageLimit: 24,
      rawWindowLimit: 120,
    });

    expect(calls).toEqual(['30', '20']);
    expect(result.chunk.map((message) => message.id)).toEqual(['10', '11', '20', '21', '30', '31']);
    expect(result.pageHasOlder).toBe(false);
    expect(result.pageHasNewer).toBe(false);
    expect(result.boundaryPage.total).toBe(99);
    expect(result.boundaryPage.last_active).toBe('latest');
  });

  test('does not over-read once the raw message window limit is reached', async () => {
    const firstPage: ChatHistoryPageRaw = { data: [{ id: 30, role: 'tool' }, { id: 31, role: 'assistant' }], has_older: true };
    const result = await backfillOlderChunkToTurnBoundary({
      firstPage,
      firstChunk: normalizeChatHistoryChunk(firstPage.data, normalize),
      fetchBefore: async () => ({ data: [{ id: 20, role: 'assistant' }], has_older: true }),
      normalizeChunk: (items) => normalizeChatHistoryChunk(items, normalize),
      numericId,
      pageLimit: 24,
      rawWindowLimit: 2,
    });

    expect(result.chunk.map((message) => message.id)).toEqual(['30', '31']);
  });

  test('stops older backfill when an overlapping page contributes no new messages', async () => {
    const calls: string[] = [];
    const firstPage: ChatHistoryPageRaw = { data: [{ id: 30, role: 'tool' }, { id: 31, role: 'assistant' }], has_older: true, has_newer: false };
    const result = await backfillOlderChunkToTurnBoundary({
      firstPage,
      firstChunk: normalizeChatHistoryChunk(firstPage.data, normalize),
      fetchBefore: async (before) => { calls.push(before); return { data: [{ id: 30, role: 'tool' }], has_older: true }; },
      normalizeChunk: (items) => normalizeChatHistoryChunk(items, normalize),
      numericId,
      pageLimit: 24,
      rawWindowLimit: 120,
    });

    expect(calls).toEqual(['30']);
    expect(result.chunk.map((message) => message.id)).toEqual(['30', '31']);
    expect(result.pageHasOlder).toBe(false);
  });
});
