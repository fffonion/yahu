export type UsageTotals = {
  sessions: number;
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  reasoning: number;
  api_calls: number;
  tool_calls: number;
  estimated_cost_usd: number;
  actual_cost_usd: number;
  total_tokens: number;
  cache_hit_rate: number;
  avg_tokens_per_session: number;
};

export type UsageDay = { date: string; label: string; totals: UsageTotals };
export type UsageModel = { model: string; totals: UsageTotals; daily: UsageDay[] };
export type UsagePeriod = { days: number; totals: UsageTotals; models: UsageModel[] };
export type UsageSource = { source: string; totals: UsageTotals };
export type UsageInsights = {
  object: string;
  generated_at: number;
  window_days: number;
  totals: UsageTotals;
  daily: UsageDay[];
  models: UsageModel[];
  sources: UsageSource[];
  periods: UsagePeriod[];
};

export type UsageMetric = 'total_tokens' | 'input' | 'output' | 'cache_read' | 'cache_write' | 'reasoning';

export const metricLabels: Record<UsageMetric, string> = {
  total_tokens: 'Total',
  input: 'Input',
  output: 'Output',
  cache_read: 'Cache read',
  cache_write: 'Cache write',
  reasoning: 'Reasoning',
};

export function fmtTokens(value: number | undefined): string {
  const n = Number(value || 0);
  if (Math.abs(n) >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (Math.abs(n) >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (Math.abs(n) >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return Math.round(n).toString();
}

export function fmtMoney(value: number | undefined): string {
  const n = Number(value || 0);
  if (!Number.isFinite(n) || n <= 0) return '$0.00';
  if (n < 0.01) return `$${n.toFixed(4)}`;
  return `$${n.toFixed(2)}`;
}

export function fmtPercent(value: number | undefined): string {
  return `${Math.round(Number(value || 0) * 100)}%`;
}

export function periodSlice(days: UsageDay[], period: number): UsageDay[] {
  return days.slice(Math.max(0, days.length - period));
}

export function modelPeriodTotals(model: UsageModel, period: number): UsageTotals {
  return periodSlice(model.daily || [], period).reduce((acc, day) => addTotals(acc, day.totals), emptyTotals());
}

export function addTotals(a: UsageTotals, b: UsageTotals): UsageTotals {
  return {
    sessions: a.sessions + (b.sessions || 0),
    input: a.input + (b.input || 0),
    output: a.output + (b.output || 0),
    cache_read: a.cache_read + (b.cache_read || 0),
    cache_write: a.cache_write + (b.cache_write || 0),
    reasoning: a.reasoning + (b.reasoning || 0),
    api_calls: a.api_calls + (b.api_calls || 0),
    tool_calls: a.tool_calls + (b.tool_calls || 0),
    estimated_cost_usd: a.estimated_cost_usd + (b.estimated_cost_usd || 0),
    actual_cost_usd: a.actual_cost_usd + (b.actual_cost_usd || 0),
    total_tokens: a.total_tokens + (b.total_tokens || 0),
    cache_hit_rate: 0,
    avg_tokens_per_session: 0,
  };
}

export function finalizeTotals(t: UsageTotals): UsageTotals {
  const inputAndCache = t.input + t.cache_read;
  return {
    ...t,
    cache_hit_rate: inputAndCache > 0 ? t.cache_read / inputAndCache : 0,
    avg_tokens_per_session: t.sessions > 0 ? t.total_tokens / t.sessions : 0,
  };
}

export function emptyTotals(): UsageTotals {
  return { sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, estimated_cost_usd: 0, actual_cost_usd: 0, total_tokens: 0, cache_hit_rate: 0, avg_tokens_per_session: 0 };
}

export function metricValue(day: UsageDay, metric: UsageMetric): number {
  return Number(day?.totals?.[metric] || 0);
}

export function linePath(values: number[], width: number, height: number, pad = 12): string {
  if (!values.length) return '';
  const max = Math.max(1, ...values);
  const innerW = Math.max(1, width - pad * 2);
  const innerH = Math.max(1, height - pad * 2);
  return values.map((value, index) => {
    const x = pad + (values.length === 1 ? innerW : (innerW * index) / (values.length - 1));
    const y = pad + innerH - (value / max) * innerH;
    return `${index === 0 ? 'M' : 'L'} ${x.toFixed(2)} ${y.toFixed(2)}`;
  }).join(' ');
}

export function areaPath(values: number[], width: number, height: number, pad = 12): string {
  const path = linePath(values, width, height, pad);
  if (!path) return '';
  const innerW = Math.max(1, width - pad * 2);
  const baseY = height - pad;
  const lastX = pad + innerW;
  return `${path} L ${lastX.toFixed(2)} ${baseY.toFixed(2)} L ${pad.toFixed(2)} ${baseY.toFixed(2)} Z`;
}
