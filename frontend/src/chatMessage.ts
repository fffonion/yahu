import { parsePlatformSenderMessage } from './chatSender';
import type { ChatMessage, ChatTurnMetrics } from './ChatTranscript';
import { normalizeMessageParts } from './messageReasoning';
import type { TurnDetailMetadata } from './turnDetails';

function asRecordish(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
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

function readTurnDetails(raw: any): TurnDetailMetadata | undefined {
  const detail = asRecordish(raw?.turnDetails) || asRecordish(raw?.turn_details);
  if (!detail) return undefined;
  const count = Number(detail.count || 0);
  if (!Number.isFinite(count) || count <= 0) return undefined;
  const out: TurnDetailMetadata = { count };
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

export function normalizeChatMessage(raw: any, fallbackId: string): ChatMessage {
  const parts = normalizeMessageParts(raw.content, raw);
  const platformSender = raw.role === 'user' ? parsePlatformSenderMessage(parts.content) : { content: parts.content };
  const msg: ChatMessage = {
    id: String(raw.id || fallbackId),
    role: ['user', 'assistant', 'tool', 'system'].includes(raw.role) ? raw.role : 'system',
    content: platformSender.content,
    reasoning: parts.reasoning,
    timestamp: raw.timestamp,
    toolName: rawToolName(raw),
    toolInput: rawToolInput(raw),
    toolCalls: raw.toolCalls ?? raw.tool_calls,
    toolCallId: String(raw.toolCallId || raw.tool_call_id || raw.call_id || '').trim() || undefined,
  };
  const tokenCount = readTokenCount(raw);
  if (tokenCount !== undefined) msg.tokenCount = tokenCount;
  const metrics = readTurnMetrics(raw);
  if (metrics) msg.turnMetrics = metrics;
  const turnDetails = readTurnDetails(raw);
  if (turnDetails) msg.turnDetails = turnDetails;
  if (platformSender.senderName) msg.platformSenderName = platformSender.senderName;
  if (platformSender.senderId) msg.platformSenderId = platformSender.senderId;
  if (typeof raw.model === 'string' && raw.model.trim()) msg.model = raw.model.trim();
  if (typeof raw.provider === 'string' && raw.provider.trim()) msg.provider = raw.provider.trim();
  return msg;
}
