type MessageParts = { content: string; reasoning: string };

const REASONING_PART_TYPES = new Set(['reasoning', 'thinking', 'thought', 'thoughts', 'analysis', 'reasoning_text', 'thinking_text']);
const VISIBLE_TEXT_PART_TYPES = new Set(['text', 'output_text', 'input_text', 'message', 'content', 'assistant']);
const REASONING_FIELD_NAMES = ['reasoning', 'reasoning_content', 'thinking', 'thinking_content', 'thought', 'thoughts', 'analysis'];

function pushUniqueText(target: string[], text: string) {
  const normalized = text.trim();
  if (!normalized) return;
  if (!target.includes(normalized)) target.push(normalized);
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function textFromUnknown(value: unknown): string {
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  if (value === null || value === undefined) return '';
  if (Array.isArray(value)) return value.map(textFromUnknown).filter(Boolean).join('\n');
  const record = asRecord(value);
  if (record) {
    for (const key of ['text', 'content', 'value', 'summary']) {
      const text = textFromUnknown(record[key]);
      if (text) return text;
    }
  }
  try { return JSON.stringify(value); } catch { return String(value); }
}

function collectReasoningFields(raw?: Record<string, unknown> | null): string[] {
  if (!raw) return [];
  const reasoning = REASONING_FIELD_NAMES.reduce<string[]>((acc, key) => {
    pushUniqueText(acc, textFromUnknown(raw[key]));
    return acc;
  }, []);
  const structuredValue = (value: unknown) => {
    if (typeof value !== 'string') return value;
    const trimmed = value.trim();
    if (!trimmed.startsWith('[') && !trimmed.startsWith('{')) return value;
    try { return JSON.parse(trimmed); } catch { return value; }
  };
  const collectThinking = (value: unknown) => {
    const parsed = structuredValue(value);
    if (Array.isArray(parsed)) { parsed.forEach(collectThinking); return; }
    const record = asRecord(parsed);
    if (!record) return;
    pushUniqueText(reasoning, textFromUnknown(record.thinking));
    if (String(record.type || '').toLowerCase().includes('reason')) pushUniqueText(reasoning, textFromUnknown(record.text));
  };
  const collectCodexSummary = (value: unknown) => {
    const parsed = structuredValue(value);
    if (Array.isArray(parsed)) { parsed.forEach(collectCodexSummary); return; }
    const record = asRecord(parsed);
    if (!record) return;
    pushUniqueText(reasoning, textFromUnknown(record.summary));
  };
  collectThinking(raw.reasoning_details ?? raw.reasoningDetails);
  collectCodexSummary(raw.codex_reasoning_items ?? raw.codexReasoningItems);
  return reasoning;
}

export function normalizeMessageParts(value: unknown, raw?: Record<string, unknown> | null): MessageParts {
  const reasoning = collectReasoningFields(raw);
  if (Array.isArray(value)) {
    const visible: string[] = [];
    for (const part of value) {
      const record = asRecord(part);
      const type = String(record?.type || record?.kind || '').toLowerCase();
      const text = textFromUnknown(record || part).trim();
      if (!text) continue;
      if (REASONING_PART_TYPES.has(type)) pushUniqueText(reasoning, text);
      else if (!type || VISIBLE_TEXT_PART_TYPES.has(type) || record?.text !== undefined || record?.content !== undefined) visible.push(text);
      else visible.push(text);
    }
    return { content: visible.join('\n'), reasoning: reasoning.join('\n') };
  }
  return { content: textFromUnknown(value), reasoning: reasoning.join('\n') };
}
