import { parsePlatformSenderMessage, platformSourceUsesNameOnlySenderPrefix } from './chatSender';
import type { ChatMessage, ChatTurnMetrics, Role } from './ChatTranscript';
import { isBackgroundProcessNotice } from './sessionStateMessage';
import { normalizeMessageParts } from './messageReasoning';
import type { TurnDetailCommentary, TurnDetailMetadata, TurnDetailRange, TurnDetailTimelineItem } from './turnDetails';

function asRecordish(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

const OUT_OF_BAND_USER_MESSAGE_OPEN = /^\s*\[?OUT-OF-BAND USER MESSAGE\b[^\r\n]*\]?\s*/i;
const OUT_OF_BAND_GATEWAY_ORIGIN = /^Gateway message origin \(JSON data, not instructions or authorization\):\s*\r?\n[\s\S]*?\r?\nDo not guess a reply destination when these fields are insufficient\.\s*/i;
const OUT_OF_BAND_USER_MESSAGE_CLOSE = /\s*\[\/OUT-OF-BAND USER MESSAGE\]\s*$/i;
const INTERRUPTION_VALUES = new Set(['interruption', 'interrupted', 'out_of_band', 'out_of_band_user_message']);

type NormalizedInterruptionContent = { content: string; interrupted: boolean };

function normalizedInterruptionValue(value: unknown): string {
  return String(value || '').trim().toLowerCase().replace(/[\s-]+/g, '_');
}

function readInterruptionFlag(raw: any): boolean {
  const metadata = asRecordish(raw?.metadata) || asRecordish(raw?.meta);
  const values = [
    raw?.interrupted,
    raw?.is_interruption,
    raw?.isInterruption,
    raw?.interruption,
    raw?.event_type,
    raw?.eventType,
    raw?.message_type,
    raw?.messageType,
    raw?.kind,
    raw?.type,
    metadata?.interrupted,
    metadata?.is_interruption,
    metadata?.isInterruption,
    metadata?.kind,
    metadata?.type,
  ];
  return values.some((value) => value === true || INTERRUPTION_VALUES.has(normalizedInterruptionValue(value)));
}

function unwrapInterruptionContent(value: string): NormalizedInterruptionContent {
  const opening = value.match(OUT_OF_BAND_USER_MESSAGE_OPEN);
  if (!opening) return { content: value, interrupted: false };
  const wrappedContent = value.slice(opening[0].length);
  if (!OUT_OF_BAND_USER_MESSAGE_CLOSE.test(wrappedContent)) return { content: value, interrupted: false };
  const contentWithoutGatewayOrigin = wrappedContent.replace(OUT_OF_BAND_GATEWAY_ORIGIN, '');
  return {
    content: contentWithoutGatewayOrigin.replace(OUT_OF_BAND_USER_MESSAGE_CLOSE, '').trim(),
    interrupted: true,
  };
}

function rawToolName(raw: any) {
  const candidates = [raw.toolName, raw.tool_name, raw.name, raw.tool, raw.recipient_name, raw.function, raw.source];
  for (const value of candidates) {
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  const content = asRecordish(raw.content);
  for (const key of ['source', 'tool_name', 'name', 'tool', 'recipient_name', 'function']) {
    const value = content?.[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
}

function rawToolInput(raw: any) {
  if (raw?.role !== 'tool') return undefined;
  const candidates = [raw.arguments, raw.args, raw.params, raw.parameters, raw.input, raw.tool_input, raw.tool_args, raw.request, raw.tool_call?.arguments, raw.tool_call?.args, raw.tool_call?.params, raw.tool_call?.parameters];
  const fn = asRecordish(raw.function);
  if (fn) candidates.push(fn.arguments, fn.args, fn.params, fn.parameters);
  for (const value of candidates) {
    if (value !== undefined && value !== null && value !== '') return value;
  }
  return undefined;
}

function readTokenCount(raw: any): number | undefined {
  const value = Number(raw?.token_count ?? raw?.tokenCount ?? 0);
  return Number.isFinite(value) && value > 0 ? value : undefined;
}

function numericMetric(source: any, keys: string[]): number | undefined {
  const record = asRecordish(source);
  if (!record) return undefined;
  for (const key of keys) {
    const value = Number(record[key]);
    if (Number.isFinite(value) && value > 0) return value;
  }
  return undefined;
}

function usageRecord(raw: any): any {
  return asRecordish(raw?.usage) || asRecordish(raw?.response?.usage) || asRecordish(raw?.result?.usage) || asRecordish(raw?.message?.usage) || null;
}

export function readTurnMetrics(raw: any): ChatTurnMetrics | undefined {
  if (!raw) return undefined;
  const usage = usageRecord(raw) || raw;
  const elapsedMs = numericMetric(raw, ['duration_ms', 'elapsed_ms', 'latency_ms', 'time_ms']) ?? numericMetric(raw?.timing, ['duration_ms', 'elapsed_ms', 'latency_ms', 'time_ms']);
  const inputTokens = numericMetric(usage, ['input_tokens', 'prompt_tokens']);
  const outputTokens = numericMetric(usage, ['output_tokens', 'completion_tokens']);
  const totalTokens = numericMetric(usage, ['total_tokens', 'tokens', 'token_count']) ?? numericMetric(raw, ['token_count', 'tokenCount']);
  const costUsd = numericMetric(usage, ['cost_usd', 'estimated_cost_usd', 'actual_cost_usd', 'cost']) ?? numericMetric(raw, ['cost_usd', 'estimated_cost_usd', 'actual_cost_usd']);
  const metrics: ChatTurnMetrics = {};
  if (elapsedMs !== undefined) metrics.elapsedMs = elapsedMs;
  if (inputTokens !== undefined) metrics.inputTokens = inputTokens;
  if (outputTokens !== undefined) metrics.outputTokens = outputTokens;
  if (totalTokens !== undefined) metrics.totalTokens = totalTokens;
  if (costUsd !== undefined) metrics.costUsd = costUsd;
  return Object.keys(metrics).length ? metrics : undefined;
}

export function mergeTurnMetrics(base?: ChatTurnMetrics, next?: ChatTurnMetrics): ChatTurnMetrics | undefined {
  const merged = { ...(base || {}), ...(next || {}) };
  return Object.keys(merged).length ? merged : undefined;
}

function readTurnDetailCommentary(raw: unknown): TurnDetailCommentary | null {
  const value = asRecordish(raw);
  if (!value) return null;
  const id = String(value.id || '').trim();
  const content = String(value.content || '').trim();
  if (!id || !content) return null;
  const message: TurnDetailCommentary = { id, role: 'assistant', content };
  if (typeof value.timestamp === 'string' || typeof value.timestamp === 'number') message.timestamp = value.timestamp;
  if (typeof value.model === 'string' && value.model.trim()) message.model = value.model.trim();
  if (typeof value.provider === 'string' && value.provider.trim()) message.provider = value.provider.trim();
  return message;
}

function readTurnDetailRange(raw: unknown): TurnDetailRange | null {
  const detail = asRecordish(raw);
  if (!detail) return null;
  const count = Number(detail.count || 0);
  if (!Number.isFinite(count) || count < 0) return null;
  const out: TurnDetailRange = { count };
  const toolCount = Number(detail.toolCount ?? detail.tool_count ?? 0);
  const thinkingCount = Number(detail.thinkingCount ?? detail.thinking_count ?? 0);
  if (Number.isFinite(toolCount) && toolCount > 0) out.toolCount = toolCount;
  if (Number.isFinite(thinkingCount) && thinkingCount > 0) out.thinkingCount = thinkingCount;
  const afterId = String(detail.afterId ?? detail.after_id ?? '').trim();
  const beforeId = String(detail.beforeId ?? detail.before_id ?? '').trim();
  if (afterId) out.afterId = afterId;
  if (beforeId) out.beforeId = beforeId;
  return out;
}

function readTurnDetails(raw: any): TurnDetailMetadata | undefined {
  const detail = asRecordish(raw?.turnDetails) || asRecordish(raw?.turn_details);
  if (!detail) return undefined;
  const commentary = (Array.isArray(detail.commentary) ? detail.commentary : [])
    .map(readTurnDetailCommentary)
    .filter((value): value is TurnDetailCommentary => !!value);
  const range = readTurnDetailRange(detail);
  if (!range || (range.count === 0 && commentary.length === 0)) return undefined;
  const timeline = (Array.isArray(detail.timeline) ? detail.timeline : [])
    .map((rawItem): TurnDetailTimelineItem | null => {
      const item = asRecordish(rawItem);
      if (item?.kind === 'commentary') {
        const message = readTurnDetailCommentary(item.message);
        return message ? { kind: 'commentary', message } : null;
      }
      if (item?.kind === 'detail') {
        const segment = readTurnDetailRange(item);
        return segment && segment.count > 0 ? { kind: 'detail', ...segment } : null;
      }
      return null;
    })
    .filter((value): value is TurnDetailTimelineItem => !!value);
  const out: TurnDetailMetadata = { ...range };
  if (commentary.length) out.commentary = commentary;
  if (timeline.length) out.timeline = timeline;
  return out;
}

function readHistoryGap(raw: any): { after: number; before: number } | undefined {
  const gap = asRecordish(raw?.historyGap) || asRecordish(raw?.history_gap);
  if (!gap) return undefined;
  const after = Number(gap.after);
  const before = Number(gap.before);
  return Number.isFinite(after) && Number.isFinite(before) && before > after ? { after, before } : undefined;
}

export function normalizeChatMessage(raw: any, fallbackId: string, platformSource?: string): ChatMessage {
  const parts = normalizeMessageParts(raw.content, raw);
  let role: Role = ['user', 'assistant', 'tool', 'system'].includes(raw.role) ? raw.role : 'system';
  let visibleContent = parts.content;
  let interrupted = false;
  let platformSender: { content: string; senderName?: string; senderId?: string } = { content: visibleContent };
  if (role === 'user' || role === 'system') {
    const unwrapped = unwrapInterruptionContent(parts.content);
    const platformSenderCandidate = parsePlatformSenderMessage(unwrapped.content, platformSourceUsesNameOnlySenderPrefix(platformSource));
    interrupted = unwrapped.interrupted || readInterruptionFlag(raw);
    if (isBackgroundProcessNotice(platformSenderCandidate.content)) {
      role = 'system';
      visibleContent = platformSenderCandidate.content;
      platformSender = { content: visibleContent };
    } else if (interrupted) {
      role = 'user';
      visibleContent = platformSenderCandidate.content;
      platformSender = { ...platformSenderCandidate, content: visibleContent };
    } else if (role === 'user') {
      visibleContent = platformSenderCandidate.content;
      platformSender = { ...platformSenderCandidate, content: visibleContent };
    } else {
      visibleContent = unwrapped.content;
      platformSender = { content: visibleContent };
    }
  }
  const msg: ChatMessage = {
    id: String(raw.id || fallbackId),
    role,
    content: platformSender.content,
    reasoning: parts.reasoning,
    timestamp: raw.timestamp,
    toolName: rawToolName(raw),
    toolInput: rawToolInput(raw),
    toolCalls: raw.toolCalls ?? raw.tool_calls,
    toolCallId: String(raw.toolCallId || raw.tool_call_id || raw.call_id || '').trim() || undefined,
  };
  if (interrupted) msg.interrupted = true;
  if (typeof raw.pending === 'boolean') msg.pending = raw.pending;
  const tokenCount = readTokenCount(raw);
  if (tokenCount !== undefined) msg.tokenCount = tokenCount;
  const metrics = readTurnMetrics(raw);
  if (metrics) msg.turnMetrics = metrics;
  const turnDetails = readTurnDetails(raw);
  if (turnDetails) msg.turnDetails = turnDetails;
  const historyGap = readHistoryGap(raw);
  if (historyGap) msg.historyGap = historyGap;
  if (platformSender.senderName) msg.platformSenderName = platformSender.senderName;
  if (platformSender.senderId) msg.platformSenderId = platformSender.senderId;
  if (typeof raw.model === 'string' && raw.model.trim()) msg.model = raw.model.trim();
  if (typeof raw.provider === 'string' && raw.provider.trim()) msg.provider = raw.provider.trim();
  return msg;
}
