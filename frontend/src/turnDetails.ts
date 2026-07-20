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

export type TurnDetailMetadata = {
  count: number;
  toolCount?: number;
  thinkingCount?: number;
  afterId?: string;
  beforeId?: string;
  commentary?: TurnDetailCommentary[];
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
  let buffer: Array<{ message: T; index: number }> = [];

  const resetBuffer = () => {
    buffer = [];
    activeAnchorId = '';
  };

  const bufferedDetailGroupId = (finalMessage: T, finalIndex: number, detail?: TurnDetailMetadata) => {
    const anchor = activeAnchorId || detail?.afterId || ROOTLESS_ANCHOR_ID;
    if (anchor !== ROOTLESS_ANCHOR_ID) return `turn-details:${anchor}`;
    const first = buffer[0];
    return `turn-details:${ROOTLESS_ANCHOR_ID}:${messageId(first?.message || finalMessage, first?.index ?? finalIndex)}`;
  };

  const pushBufferedDetailGroup = (id: string, finalMessage: T, finalIndex: number, detail?: TurnDetailMetadata, options?: { defaultOpen?: boolean }) => {
    items.push({
      kind: 'detailGroup',
      id,
      messages: buffer.map((entry) => entry.message),
      sourceIndexes: buffer.map((entry) => entry.index),
      finalMessage,
      finalIndex,
      detail,
      defaultOpen: options?.defaultOpen || undefined,
    });
    resetBuffer();
  };

  const flushBufferAsMessages = () => {
    for (const entry of buffer) items.push({ kind: 'message', message: entry.message, sourceIndexes: [entry.index] });
    resetBuffer();
  };

  const flushTrailingBuffer = () => {
    if (buffer.length) {
      const last = buffer[buffer.length - 1];
      const isUnfinishedUserTurn = (!!activeAnchorId && activeAnchorId !== ROOTLESS_ANCHOR_ID) || !!options.openTrailingDetails;
      pushBufferedDetailGroup(bufferedDetailGroupId(last.message, last.index), last.message, last.index, undefined, { defaultOpen: isUnfinishedUserTurn });
      return;
    }
    flushBufferAsMessages();
  };

  const pushDetailGroup = (finalMessage: T, finalIndex: number) => {
    const detail = turnDetailMetadata(finalMessage);
    if (buffer.length || detail) pushBufferedDetailGroup(bufferedDetailGroupId(finalMessage, finalIndex, detail), finalMessage, finalIndex, detail);
    items.push({ kind: 'message', message: finalMessage, sourceIndexes: [finalIndex] });
    activeAnchorId = '';
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
      flushBufferAsMessages();
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
      items.push({ kind: 'message', message, sourceIndexes: [index] });
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
    if (!current || splitAfterHistoryGap || item.kind === 'sessionState' || (item.kind === 'message' && (item.message.role === 'user' || !!item.message.historyGap))) {
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
