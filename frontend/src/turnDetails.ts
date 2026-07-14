import { isAssistantToolPreludeMessage, isToolLikeMessage, type MessageVisibilityInput } from './messageVisibility';
import { isSessionStateMessage } from './sessionStateMessage';

export type TurnDetailMessageItem<T> = {
  kind: 'message';
  message: T;
  sourceIndexes: number[];
};

export type TurnDetailMetadata = {
  count: number;
  toolCount?: number;
  thinkingCount?: number;
  afterId?: string;
  beforeId?: string;
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
  return detail && Number(detail.count || 0) > 0 ? detail : undefined;
}

export function buildTurnDetailItems<T extends MessageVisibilityInput>(messages: T[]): Array<TurnDetailItem<T>> {
  const items: Array<TurnDetailItem<T>> = [];
  let activeAnchorId = '';
  let buffer: Array<{ message: T; index: number }> = [];

  const resetBuffer = () => {
    buffer = [];
    activeAnchorId = '';
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
    if (buffer.length && !buffer.some((entry) => entry.message.pending)) {
      const last = buffer[buffer.length - 1];
      const isUnfinishedUserTurn = !!activeAnchorId && activeAnchorId !== ROOTLESS_ANCHOR_ID;
      const suffix = isUnfinishedUserTurn ? `unfinished-${messageId(last.message, last.index)}` : `trailing-${messageId(last.message, last.index)}`;
      pushBufferedDetailGroup(`turn-details:${activeAnchorId || ROOTLESS_ANCHOR_ID}:${suffix}`, last.message, last.index, undefined, { defaultOpen: isUnfinishedUserTurn });
      return;
    }
    flushBufferAsMessages();
  };

  const pushDetailGroup = (finalMessage: T, finalIndex: number) => {
    const detail = turnDetailMetadata(finalMessage);
    if (buffer.length || detail) pushBufferedDetailGroup(`turn-details:${activeAnchorId || detail?.afterId || ROOTLESS_ANCHOR_ID}:${messageId(finalMessage, finalIndex)}`, finalMessage, finalIndex, detail);
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

export function buildDesktopTurnBlocks<T extends MessageVisibilityInput>(items: Array<TurnDetailItem<T>>): Array<TurnDetailBlock<T>> {
  const blocks: Array<TurnDetailBlock<T>> = [];
  let current: TurnDetailBlock<T> | null = null;
  const append = (item: TurnDetailItem<T>) => {
    const sourceIndexes = item.sourceIndexes || [];
    if (!current || item.kind === 'sessionState' || (item.kind === 'message' && item.message.role === 'user')) {
      const firstId = item.kind === 'message' ? messageId(item.message, sourceIndexes[0] ?? blocks.length) : item.id;
      current = { id: `desktop-turn:${firstId}:${blocks.length}`, items: [], sourceIndexes: [] };
      blocks.push(current);
    }
    current.items.push(item);
    current.sourceIndexes.push(...sourceIndexes);
  };
  items.forEach(append);
  return blocks;
}
