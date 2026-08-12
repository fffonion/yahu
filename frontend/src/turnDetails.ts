import { isAssistantToolPreludeMessage, isToolLikeMessage, type MessageVisibilityInput } from './messageVisibility';
import { isSessionStateMessage } from './sessionStateMessage';

export type TurnDetailMessageItem<T> = {
  kind: 'message';
  message: T;
  sourceIndexes: number[];
};

export type TurnDetailCommentary = {
  id: string;
  role: 'assistant';
  content: string;
  timestamp?: string | number;
  model?: string;
  provider?: string;
};

export type TurnDetailRange = {
  count: number;
  toolCount?: number;
  thinkingCount?: number;
  afterId?: string;
  beforeId?: string;
};

export type TurnDetailTimelineItem =
  | { kind: 'commentary'; message: TurnDetailCommentary }
  | ({ kind: 'detail' } & TurnDetailRange);

export type TurnDetailMetadata = TurnDetailRange & {
  commentary?: TurnDetailCommentary[];
  timeline?: TurnDetailTimelineItem[];
};

export type TurnDetailGroupItem<T> = {
  kind: 'detailGroup';
  id: string;
  messages: T[];
  sourceIndexes: number[];
  finalMessage: T;
  finalIndex: number;
  detail?: TurnDetailMetadata;
  defaultOpen?: boolean;
};

export type SpecialContextGroupItem<T> = {
  kind: 'specialContextGroup';
  id: string;
  messages: T[];
  sourceIndexes: number[];
};

export type SessionStateMessageItem<T> = {
  kind: 'sessionState';
  id: string;
  message: T;
  sourceIndexes: number[];
};

export type TurnDetailBlock<T> = {
  id: string;
  items: Array<TurnDetailItem<T>>;
  sourceIndexes: number[];
};

export type TurnDetailItem<T> = TurnDetailMessageItem<T> | TurnDetailGroupItem<T> | SpecialContextGroupItem<T> | SessionStateMessageItem<T>;

type MessageWithTurnDetails = MessageVisibilityInput & { turnDetails?: TurnDetailMetadata };

const ROOTLESS_ANCHOR_ID = 'rootless';

function messageId(message: MessageVisibilityInput, fallback: number) {
  return String(message.id || fallback).trim() || String(fallback);
}

function isCompletedFinalAssistant(message: MessageVisibilityInput) {
  return message.role === 'assistant'
    && !message.pending
    && !!String(message.content || '').trim()
    && !isHermesSpecialContextMessage(message)
    && !isAssistantToolPreludeMessage(message)
    && !isToolLikeMessage(message);
}

export function isHermesSpecialContextMessage(message: MessageVisibilityInput): boolean {
  if (message.role !== 'assistant' || message.pending) return false;
  const text = String(message.content || '').trimStart();
  if (!text) return false;
  const firstLine = text.split(/\r?\n/, 1)[0]?.trim() || '';
  return /^\[PRIOR CONTEXT\s*(?:--|—)/i.test(firstLine)
    || /^\[CONTEXT COMPACTION\s*(?:--|—)\s*REFERENCE ONLY\]/i.test(firstLine);
}

function isRootlessDetailCandidate(message: MessageVisibilityInput) {
  return isAssistantToolPreludeMessage(message) || isToolLikeMessage(message);
}

function turnDetailMetadata(message: MessageVisibilityInput): TurnDetailMetadata | undefined {
  const detail = (message as MessageWithTurnDetails).turnDetails;
  return detail && (Number(detail.count || 0) > 0 || !!detail.commentary?.length) ? detail : undefined;
}

export function buildTurnDetailItems<T extends MessageVisibilityInput>(
  messages: T[],
  options: { openTrailingDetails?: boolean } = {},
): Array<TurnDetailItem<T>> {
  const items: Array<TurnDetailItem<T>> = [];
  let activeAnchorId = '';
  let activeDetailSegment = 0;
  let activeSegmentBoundaryId = '';
  let buffer: Array<{ message: T; index: number }> = [];

  const clearBuffer = () => {
    buffer = [];
  };

  const resetTurn = () => {
    clearBuffer();
    activeAnchorId = '';
    activeDetailSegment = 0;
    activeSegmentBoundaryId = '';
  };

  const bufferedDetailGroupId = (finalMessage: T, finalIndex: number, detail?: TurnDetailMetadata) => {
    const anchor = activeAnchorId || detail?.afterId || ROOTLESS_ANCHOR_ID;
    const first = buffer[0];
    const firstId = messageId(first?.message || finalMessage, first?.index ?? finalIndex);
    const base = anchor === ROOTLESS_ANCHOR_ID
      ? `turn-details:${ROOTLESS_ANCHOR_ID}:${firstId}`
      : `turn-details:${anchor}`;
    if (activeDetailSegment === 0) return base;
    const boundary = detail?.afterId || activeSegmentBoundaryId || firstId;
    return `${base}:segment:${boundary}`;
  };

  const pushBufferedDetailGroup = (
    id: string,
    finalMessage: T,
    finalIndex: number,
    detail?: TurnDetailMetadata,
    options?: { defaultOpen?: boolean; preserveTurn?: boolean; beforeLastFinal?: boolean },
  ) => {
    const savedAnchor = activeAnchorId;
    const group: TurnDetailGroupItem<T> = {
      kind: 'detailGroup',
      id,
      messages: buffer.map((entry) => entry.message),
      sourceIndexes: buffer.length ? buffer.map((entry) => entry.index) : [finalIndex],
      finalMessage,
      finalIndex,
      detail,
      defaultOpen: options?.defaultOpen || undefined,
    };
    if (options?.beforeLastFinal) {
      let lastFinalIndex = -1;
      items.forEach((item, index) => {
        if (item.kind === 'message' && isCompletedFinalAssistant(item.message)) lastFinalIndex = index;
      });
      if (lastFinalIndex >= 0) items.splice(lastFinalIndex, 0, group);
      else items.push(group);
    } else {
      items.push(group);
    }
    clearBuffer();
    if (options?.preserveTurn) {
      activeAnchorId = savedAnchor;
      activeDetailSegment += 1;
    } else {
      resetTurn();
    }
  };

  const flushBufferAsMessages = () => {
    for (const entry of buffer) items.push({ kind: 'message', message: entry.message, sourceIndexes: [entry.index] });
    resetTurn();
  };

  const flushBufferAsDetailGroup = () => {
    if (!buffer.length) return;
    const last = buffer[buffer.length - 1];
    pushBufferedDetailGroup(
      bufferedDetailGroupId(last.message, last.index),
      last.message,
      last.index,
      undefined,
      { defaultOpen: !!options.openTrailingDetails, preserveTurn: true },
    );
  };

  const flushTrailingBuffer = () => {
    if (buffer.length) {
      const last = buffer[buffer.length - 1];
      const isUnfinishedUserTurn = !!options.openTrailingDetails;
      pushBufferedDetailGroup(bufferedDetailGroupId(last.message, last.index), last.message, last.index, undefined, {
        defaultOpen: isUnfinishedUserTurn,
        beforeLastFinal: activeAnchorId === ROOTLESS_ANCHOR_ID && items.some((item) => item.kind === 'message' && isCompletedFinalAssistant(item.message)),
      });
      return;
    }
    flushBufferAsMessages();
  };

  const pushDetailGroup = (finalMessage: T, finalIndex: number) => {
    const detail = turnDetailMetadata(finalMessage);
    if (detail?.timeline?.length) {
      if (buffer.length) {
        pushBufferedDetailGroup(
          bufferedDetailGroupId(finalMessage, finalIndex),
          finalMessage,
          finalIndex,
          undefined,
          { preserveTurn: true },
        );
      }
      for (const timelineItem of detail.timeline) {
        if (timelineItem.kind === 'commentary') {
          const commentaryMessage = timelineItem.message as T;
          items.push({ kind: 'message', message: commentaryMessage, sourceIndexes: [finalIndex] });
          activeSegmentBoundaryId = messageId(commentaryMessage, finalIndex);
          continue;
        }
        const { kind: _kind, ...segmentDetail } = timelineItem;
        const normalizedDetail: TurnDetailMetadata = segmentDetail;
        pushBufferedDetailGroup(
          bufferedDetailGroupId(finalMessage, finalIndex, normalizedDetail),
          finalMessage,
          finalIndex,
          normalizedDetail,
          { preserveTurn: true },
        );
      }
      items.push({ kind: 'message', message: finalMessage, sourceIndexes: [finalIndex] });
      resetTurn();
      return;
    }
    if (buffer.length || detail) pushBufferedDetailGroup(bufferedDetailGroupId(finalMessage, finalIndex, detail), finalMessage, finalIndex, detail);
    items.push({ kind: 'message', message: finalMessage, sourceIndexes: [finalIndex] });
    resetTurn();
  };

  const pushSpecialContextGroup = (message: T, index: number, anchor: string) => {
    items.push({
      kind: 'specialContextGroup',
      id: `special-context:${anchor}:${messageId(message, index)}`,
      messages: [message],
      sourceIndexes: [index],
    });
  };

  messages.forEach((message, index) => {
    if (message.historyGap) {
      flushBufferAsMessages();
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      return;
    }

    if (isSessionStateMessage(message)) {
      flushBufferAsDetailGroup();
      activeAnchorId = messageId(message, index);
      items.push({
        kind: 'sessionState',
        id: `session-state:${messageId(message, index)}`,
        message,
        sourceIndexes: [index],
      });
      return;
    }

    if (message.role === 'user') {
      flushBufferAsMessages();
      activeAnchorId = messageId(message, index);
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      return;
    }

    if (isHermesSpecialContextMessage(message)) {
      const savedAnchor = activeAnchorId || ROOTLESS_ANCHOR_ID;
      flushBufferAsMessages();
      pushSpecialContextGroup(message, index, savedAnchor);
      return;
    }

    if (isAssistantToolPreludeMessage(message)) {
      if (buffer.length) {
        const last = buffer[buffer.length - 1];
        const keepOpen = !!options.openTrailingDetails;
        pushBufferedDetailGroup(
          bufferedDetailGroupId(last.message, last.index),
          message,
          index,
          undefined,
          { defaultOpen: keepOpen, preserveTurn: true },
        );
      }
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      activeSegmentBoundaryId = messageId(message, index);
      return;
    }

    if (isCompletedFinalAssistant(message)) {
      pushDetailGroup(message, index);
      return;
    }

    if (activeAnchorId) {
      buffer.push({ message, index });
      return;
    }

    if (isRootlessDetailCandidate(message)) {
      activeAnchorId = ROOTLESS_ANCHOR_ID;
      buffer.push({ message, index });
      return;
    }

    items.push({ kind: 'message', message, sourceIndexes: [index] });
  });

  flushTrailingBuffer();
  return items;
}

export function latestExpandableDetailGroupId<T extends MessageVisibilityInput>(
  items: Array<TurnDetailItem<T>>,
  messages: T[],
  streaming: boolean,
): string {
  let latestUserIndex = -1;
  let latestFinalIndex = -1;
  messages.forEach((message, index) => {
    if (message.role === 'user') latestUserIndex = index;
    if (isCompletedFinalAssistant(message)) latestFinalIndex = index;
  });
  const boundaryIndex = streaming ? latestUserIndex : Math.max(latestUserIndex, latestFinalIndex);
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item.kind !== 'detailGroup') continue;
    if (item.sourceIndexes.some((sourceIndex) => sourceIndex > boundaryIndex)) return item.id;
  }
  return '';
}

export function preserveTurnDetailGroupIds<T extends MessageVisibilityInput>(
  previous: Array<TurnDetailItem<T>>,
  next: Array<TurnDetailItem<T>>,
): Array<TurnDetailItem<T>> {
  const previousGroups = previous.filter((item): item is TurnDetailGroupItem<T> => item.kind === 'detailGroup');
  const claimed = new Set<number>();
  return next.map((item) => {
    if (item.kind !== 'detailGroup') return item;
    const messageIds = new Set(item.messages.map((message) => String(message.id || '').trim()).filter(Boolean));
    let matchIndex = previousGroups.findIndex((group, index) => !claimed.has(index) && group.id === item.id);
    if (matchIndex < 0 && messageIds.size > 0) {
      matchIndex = previousGroups.findIndex((group, index) => !claimed.has(index) && group.messages.some((message) => messageIds.has(String(message.id || '').trim())));
    }
    if (matchIndex < 0) return item;
    claimed.add(matchIndex);
    const preservedId = previousGroups[matchIndex].id;
    return preservedId === item.id ? item : { ...item, id: preservedId };
  });
}

export function buildDesktopTurnBlocks<T extends MessageVisibilityInput>(items: Array<TurnDetailItem<T>>): Array<TurnDetailBlock<T>> {
  const blocks: Array<TurnDetailBlock<T>> = [];
  let current: TurnDetailBlock<T> | null = null;
  let splitAfterHistoryGap = false;
  const append = (item: TurnDetailItem<T>) => {
    const sourceIndexes = item.sourceIndexes || [];
    if (!current || splitAfterHistoryGap || (item.kind === 'message' && (item.message.role === 'user' || !!item.message.historyGap))) {
      const firstId = item.kind === 'message' ? messageId(item.message, sourceIndexes[0] ?? blocks.length) : item.id;
      current = { id: `desktop-turn:${firstId}:${blocks.length}`, items: [], sourceIndexes: [] };
      blocks.push(current);
    }
    current.items.push(item);
    current.sourceIndexes.push(...sourceIndexes);
    splitAfterHistoryGap = item.kind === 'message' && !!item.message.historyGap;
  };
  items.forEach(append);
  return blocks.map((block) => {
    const detailGroup = block.items.find((item): item is TurnDetailGroupItem<T> => item.kind === 'detailGroup');
    return detailGroup ? { ...block, id: `desktop-turn:${detailGroup.id}` } : block;
  });
}
