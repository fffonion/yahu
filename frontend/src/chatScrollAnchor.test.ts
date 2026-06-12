import { describe, expect, test } from 'bun:test';
import { captureMessageScrollAnchor, restoreMessageScrollAnchor } from './chatScrollAnchor';

type FakeRow = {
  dataset: { messageId?: string };
  top: number;
  bottom: number;
  getBoundingClientRect: () => { top: number; bottom: number };
};

function row(id: string, top: number, bottom: number): FakeRow {
  return { dataset: { messageId: id }, top, bottom, getBoundingClientRect: () => ({ top, bottom }) };
}

function scroller(rows: FakeRow[], top = 100) {
  return {
    scrollTop: 250,
    getBoundingClientRect: () => ({ top }),
    querySelectorAll: () => rows,
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
});
