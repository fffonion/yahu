export type MessageScrollAnchor = { id: string; topOffset: number };

type AnchorRow = HTMLElement & { dataset: DOMStringMap };

function isClosedDetailDescendant(row: AnchorRow): boolean {
  const detail = row.closest?.('.turn-detail-group') as (HTMLDetailsElement | null);
  return !!detail && !detail.open && detail !== row;
}

function messageRows(scroller: HTMLElement): AnchorRow[] {
  return Array.from(scroller.querySelectorAll<AnchorRow>('[data-message-id]')).filter((row) => !isClosedDetailDescendant(row));
}

function messageIdSelector(id: string): string {
  const css = globalThis.CSS?.escape ? globalThis.CSS.escape(id) : id.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  return `[data-message-id="${css}"]`;
}

function rowsForMessageId(scroller: HTMLElement, id: string): AnchorRow[] {
  const matched = Array.from(scroller.querySelectorAll<AnchorRow>(messageIdSelector(id))).filter((row) => !isClosedDetailDescendant(row));
  return matched.length ? matched : messageRows(scroller).filter((row) => String(row.dataset.messageId || '') === id);
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
  const row = rowsForMessageId(scroller, anchor.id)[0];
  if (!row) return false;
  scroller.scrollTop += row.getBoundingClientRect().top - scrollerTop - anchor.topOffset;
  return true;
}
