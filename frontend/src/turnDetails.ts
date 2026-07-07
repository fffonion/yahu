import { isAssistantToolPreludeMessage, isToolLikeMessage, type MessageVisibilityInput } from './messageVisibility';

export type TurnDetailMessageItem<T> = {
  kind: 'message';
  message: T;
  sourceIndexes: number[];
};

export type TurnDetailGroupItem<T> = {
  kind: 'detailGroup';
  id: string;
  messages: T[];
  sourceIndexes: number[];
  finalMessage: T;
  finalIndex: number;
};

export type TurnDetailBlock<T> = {
  id: string;
  items: Array<TurnDetailItem<T>>;
  sourceIndexes: number[];
};

export type TurnDetailItem<T> = TurnDetailMessageItem<T> | TurnDetailGroupItem<T>;

const ROOTLESS_ANCHOR_ID = 'rootless';

function messageId(message: MessageVisibilityInput, fallback: number) {
  return String(message.id || fallback).trim() || String(fallback);
}

function isCompletedFinalAssistant(message: MessageVisibilityInput) {
  return message.role === 'assistant'
    && !message.pending
    && !!String(message.content || '').trim()
    && !isAssistantToolPreludeMessage(message)
    && !isToolLikeMessage(message);
}

function isRootlessDetailCandidate(message: MessageVisibilityInput) {
  return isAssistantToolPreludeMessage(message) || isToolLikeMessage(message);
}

export function buildTurnDetailItems<T extends MessageVisibilityInput>(messages: T[]): Array<TurnDetailItem<T>> {
  const items: Array<TurnDetailItem<T>> = [];
  let activeAnchorId = '';
  let buffer: Array<{ message: T; index: number }> = [];

  const resetBuffer = () => {
    buffer = [];
    activeAnchorId = '';
  };

  const pushBufferedDetailGroup = (id: string, finalMessage: T, finalIndex: number) => {
    items.push({
      kind: 'detailGroup',
      id,
      messages: buffer.map((entry) => entry.message),
      sourceIndexes: buffer.map((entry) => entry.index),
      finalMessage,
      finalIndex,
    });
    resetBuffer();
  };

  const flushBufferAsMessages = () => {
    for (const entry of buffer) items.push({ kind: 'message', message: entry.message, sourceIndexes: [entry.index] });
    resetBuffer();
  };

  const flushTrailingBuffer = () => {
    if (activeAnchorId === ROOTLESS_ANCHOR_ID && buffer.length && !buffer.some((entry) => entry.message.pending)) {
      const last = buffer[buffer.length - 1];
      pushBufferedDetailGroup(`turn-details:${ROOTLESS_ANCHOR_ID}:trailing-${messageId(last.message, last.index)}`, last.message, last.index);
      return;
    }
    flushBufferAsMessages();
  };

  const pushDetailGroup = (finalMessage: T, finalIndex: number) => {
    if (buffer.length) pushBufferedDetailGroup(`turn-details:${activeAnchorId || ROOTLESS_ANCHOR_ID}:${messageId(finalMessage, finalIndex)}`, finalMessage, finalIndex);
    items.push({ kind: 'message', message: finalMessage, sourceIndexes: [finalIndex] });
    activeAnchorId = '';
  };

  messages.forEach((message, index) => {
    if (message.role === 'user') {
      flushBufferAsMessages();
      activeAnchorId = messageId(message, index);
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

export function buildDesktopTurnBlocks<T extends MessageVisibilityInput>(items: Array<TurnDetailItem<T>>): Array<TurnDetailBlock<T>> {
  const blocks: Array<TurnDetailBlock<T>> = [];
  let current: TurnDetailBlock<T> | null = null;
  const append = (item: TurnDetailItem<T>) => {
    const sourceIndexes = item.sourceIndexes || [];
    if (!current || (item.kind === 'message' && item.message.role === 'user')) {
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
