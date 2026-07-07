import { describe, expect, test } from 'bun:test';
import { captureMessageScrollAnchor, restoreMessageScrollAnchor } from './chatScrollAnchor';

type FakeDetails = { open: boolean };
type FakeRow = {
  dataset: { messageId?: string };
  top: number;
  bottom: number;
  detail?: FakeDetails;
  isDetailRoot?: boolean;
  getBoundingClientRect: () => { top: number; bottom: number };
  closest: (selector: string) => FakeDetails | null;
};

function row(id: string, top: number, bottom: number, options: { detail?: FakeDetails; isDetailRoot?: boolean } = {}): FakeRow {
  return {
    dataset: { messageId: id },
    top,
    bottom,
    detail: options.detail,
    isDetailRoot: options.isDetailRoot,
    getBoundingClientRect: () => ({ top, bottom }),
    closest: (selector: string) => selector === '.turn-detail-group' && options.detail && !options.isDetailRoot ? options.detail : null,
  };
}

function scroller(rows: FakeRow[], top = 100, queries: string[] = []) {
  return {
    scrollTop: 250,
    getBoundingClientRect: () => ({ top }),
    querySelectorAll: (selector: string) => {
      queries.push(selector);
      const match = selector.match(/^\[data-message-id="(.+)"\]$/);
      if (!match) return rows;
      const id = match[1].replace(/\\"/g, '"').replace(/\\\\/g, '\\');
      return rows.filter((item) => item.dataset.messageId === id);
    },
  };
}

describe('chat scroll anchor preservation', () => {
  test('captures the first eligible message crossing the viewport top', () => {
    const anchor = captureMessageScrollAnchor(scroller([
      row('tool-1', 40, 80),
      row('answer-1', 90, 150),
      row('answer-2', 160, 240),
    ]) as any, new Set(['answer-1', 'answer-2']));

    expect(anchor).toEqual({ id: 'answer-1', topOffset: -10 });
  });

  test('restores scrollTop by the captured row delta', () => {
    const nextRows = [row('answer-1', 70, 120)];
    const fakeScroller = scroller(nextRows) as any;
    restoreMessageScrollAnchor(fakeScroller, { id: 'answer-1', topOffset: -10 });

    expect(fakeScroller.scrollTop).toBe(230);
  });

  test('uses the visible closed turn detail group instead of hidden rows inside it', () => {
    const closedDetail = { open: false };
    const anchor = captureMessageScrollAnchor(scroller([
      row('detail-first', 90, 132, { detail: closedDetail, isDetailRoot: true }),
      row('hidden-tool-1', 0, 0, { detail: closedDetail }),
      row('hidden-tool-2', 0, 0, { detail: closedDetail }),
      row('answer-1', 150, 220),
    ]) as any);

    expect(anchor).toEqual({ id: 'detail-first', topOffset: -10 });
  });

  test('restores to the visible closed turn detail group when matching hidden rows also exist', () => {
    const closedDetail = { open: false };
    const fakeScroller = scroller([
      row('detail-first', 70, 112, { detail: closedDetail, isDetailRoot: true }),
      row('detail-first', 0, 0, { detail: closedDetail }),
      row('answer-1', 140, 210),
    ]) as any;
    restoreMessageScrollAnchor(fakeScroller, { id: 'detail-first', topOffset: -10 });

    expect(fakeScroller.scrollTop).toBe(230);
  });

  test('restores by querying the captured id directly before falling back to a full row scan', () => {
    const queries: string[] = [];
    const fakeScroller = scroller([
      row('other', 10, 50),
      row('answer-1', 70, 120),
      row('answer-2', 140, 210),
    ], 100, queries) as any;

    restoreMessageScrollAnchor(fakeScroller, { id: 'answer-1', topOffset: -10 });

    expect(queries[0]).toBe('[data-message-id="answer-1"]');
    expect(queries).not.toContain('[data-message-id]');
    expect(fakeScroller.scrollTop).toBe(230);
  });
});
