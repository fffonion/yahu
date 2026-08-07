export const REASONING_EFFORTS = ['minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra'] as const;

export type ReasoningEffort = (typeof REASONING_EFFORTS)[number];

type ReasoningSessionLike = {
  reasoning_effort?: unknown;
  model_config?: unknown;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function parseModelConfig(value: unknown): Record<string, unknown> | null {
  if (typeof value === 'string') {
    try {
      return asRecord(JSON.parse(value));
    } catch {
      return null;
    }
  }
  return asRecord(value);
}

export function normalizeReasoningEffort(value: unknown): ReasoningEffort | null {
  const normalized = String(value || '').trim().toLowerCase();
  return (REASONING_EFFORTS as readonly string[]).includes(normalized) ? normalized as ReasoningEffort : null;
}

export function sessionReasoningEffort(session: ReasoningSessionLike | null | undefined): ReasoningEffort | null {
  if (!session) return null;
  const config = parseModelConfig(session.model_config);
  const reasoningConfig = asRecord(config?.reasoning_config);
  return normalizeReasoningEffort(
    session.reasoning_effort
      ?? config?.reasoning_effort
      ?? reasoningConfig?.effort,
  );
}
