export type ChatViewAnchor = { id: string; topOffset: number };

export type ChatViewState = {
  lastSessionId: string;
  positions: Record<string, number>;
  anchors: Record<string, ChatViewAnchor>;
};

export function migrateChatViewState(state: ChatViewState, previousId: string, canonicalId: string): ChatViewState {
  const next: ChatViewState = {
    lastSessionId: state.lastSessionId,
    positions: { ...state.positions },
    anchors: { ...state.anchors },
  };
  if (!previousId || !canonicalId || previousId === canonicalId) return next;

  if (Object.prototype.hasOwnProperty.call(next.positions, previousId)) {
    next.positions[canonicalId] = next.positions[previousId];
  }
  if (next.anchors[previousId]) next.anchors[canonicalId] = next.anchors[previousId];
  if (next.lastSessionId === previousId) next.lastSessionId = canonicalId;
  delete next.positions[previousId];
  delete next.anchors[previousId];
  return next;
}
