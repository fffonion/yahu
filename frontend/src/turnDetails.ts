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

export function buildTurnDetailItems<T extends MessageVisibilityInput>(messages: T[]): Array<TurnDetailItem<T>> {
  const items: Array<TurnDetailItem<T>> = [];
  let activeUserId = '';
  let buffer: Array<{ message: T; index: number }> = [];
  let turnHasFinal = false;

  const flushBufferAsMessages = () => {
    for (const entry of buffer) items.push({ kind: 'message', message: entry.message, sourceIndexes: [entry.index] });
    buffer = [];
  };

  messages.forEach((message, index) => {
    if (message.role === 'user') {
      flushBufferAsMessages();
      activeUserId = messageId(message, index);
      turnHasFinal = false;
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      return;
    }

    if (!activeUserId || turnHasFinal) {
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      return;
    }

    if (isCompletedFinalAssistant(message)) {
      if (buffer.length) {
        items.push({
          kind: 'detailGroup',
          id: `turn-details:${activeUserId}:${messageId(message, index)}`,
          messages: buffer.map((entry) => entry.message),
          sourceIndexes: buffer.map((entry) => entry.index),
          finalMessage: message,
          finalIndex: index,
        });
        buffer = [];
      }
      items.push({ kind: 'message', message, sourceIndexes: [index] });
      turnHasFinal = true;
      return;
    }

    buffer.push({ message, index });
  });

  flushBufferAsMessages();
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
