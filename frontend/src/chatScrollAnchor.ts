export type MessageScrollAnchor = { id: string; topOffset: number };

type AnchorRow = HTMLElement & { dataset: DOMStringMap };

function messageRows(scroller: HTMLElement): AnchorRow[] {
  return Array.from(scroller.querySelectorAll<AnchorRow>('[data-message-id]'));
}

export function captureMessageScrollAnchor(scroller: HTMLElement | null, eligibleIds?: Set<string>): MessageScrollAnchor | null {
  if (!scroller) return null;
  const rows = messageRows(scroller).filter((row) => String(row.dataset.messageId || '').trim());
  if (!rows.length) return null;
  const scrollerTop = scroller.getBoundingClientRect().top;
  const firstCrossingTop = (row: AnchorRow) => row.getBoundingClientRect().bottom >= scrollerTop + 1;
  const eligibleRows = eligibleIds?.size ? rows.filter((row) => eligibleIds.has(String(row.dataset.messageId || ''))) : rows;
  const row = eligibleRows.find(firstCrossingTop) || eligibleRows[0] || rows.find(firstCrossingTop) || rows[0];
  const id = String(row.dataset.messageId || '').trim();
  if (!id) return null;
  return { id, topOffset: row.getBoundingClientRect().top - scrollerTop };
}

export function restoreMessageScrollAnchor(scroller: HTMLElement | null, anchor: MessageScrollAnchor | null): boolean {
  if (!scroller || !anchor?.id) return false;
  const scrollerTop = scroller.getBoundingClientRect().top;
  const row = messageRows(scroller).find((candidate) => String(candidate.dataset.messageId || '') === anchor.id);
  if (!row) return false;
  scroller.scrollTop += row.getBoundingClientRect().top - scrollerTop - anchor.topOffset;
  return true;
}
