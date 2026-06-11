export type NewMessageMarkerInput = {
  id: string;
  pending?: boolean;
};

export type NewMessageMarker = {
  firstId: string;
  count: number;
};

function messageIdSet(messages: NewMessageMarkerInput[]) {
  return new Set(messages.map((message) => message.id));
}

export function computeNewMessageMarker<T extends NewMessageMarkerInput>(previousVisible: T[], nextVisible: T[], currentFirstId = ''): NewMessageMarker {
  let firstIdx = currentFirstId ? nextVisible.findIndex((message) => message.id === currentFirstId) : -1;
  if (firstIdx < 0) {
    const previousIds = messageIdSet(previousVisible);
    firstIdx = nextVisible.findIndex((message) => !previousIds.has(message.id));
  }
  if (firstIdx < 0) return { firstId: '', count: 0 };
  const firstId = nextVisible[firstIdx].id;
  const count = nextVisible.slice(firstIdx).filter((message) => !message.pending).length;
  return count > 0 ? { firstId, count } : { firstId: '', count: 0 };
}

export function findNewMessageSplitIndex<T extends NewMessageMarkerInput>(visibleMessages: T[], firstId = ''): number {
  if (!firstId) return -1;
  return visibleMessages.findIndex((message) => message.id === firstId);
}
