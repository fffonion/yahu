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
  cost_usd: number;
  unpriced_tokens: number;
  total_tokens: number;
  cache_hit_rate: number;
  avg_tokens_per_session: number;
};

export type UsageDay = { date: string; label: string; totals: UsageTotals };
export type UsageHour = { hour: string; label: string; totals: UsageTotals };
export type UsageModel = { model: string; totals: UsageTotals; daily: UsageDay[]; hourly?: UsageHour[] };
export type UsageSource = { source: string; totals: UsageTotals };
export type UsagePeriod = { days: number; totals: UsageTotals; models: UsageModel[]; sources?: UsageSource[] };
export type UsageInsights = {
  object: string;
  generated_at: number;
  window_days: number;
  totals: UsageTotals;
  daily: UsageDay[];
  hourly?: UsageHour[];
  models: UsageModel[];
  sources: UsageSource[];
  periods: UsagePeriod[];
  coverage_started_at?: number | null;
  coverage_complete?: boolean;
};

export type UsageMetric = 'total_tokens' | 'input' | 'output' | 'cache_read' | 'reasoning' | 'cost_usd';

export const metricLabels: Record<UsageMetric, string> = {
  total_tokens: 'Total',
  input: 'Input',
  output: 'Output',
  cache_read: 'Cache read',
  reasoning: 'Reasoning',
  cost_usd: 'Cost',
};

export function fmtTokens(value: number | undefined): string {
  const n = Number(value || 0);
  if (Math.abs(n) >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (Math.abs(n) >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (Math.abs(n) >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return Math.round(n).toString();
}

export function fmtCompactAxisTick(value: number | undefined): string {
  const n = Number(value || 0);
  const abs = Math.abs(n);
  const sign = n < 0 ? '-' : '';
  if (abs >= 1_000_000_000) return `${sign}${Math.round(abs / 1_000_000_000)}B`;
  if (abs >= 1_000_000) return `${sign}${Math.round(abs / 1_000_000)}M`;
  if (abs >= 1_000) return `${sign}${Math.round(abs / 1_000)}K`;
  return Math.round(n).toString();
}

export function fmtMoney(value: number | undefined): string {
  const n = Number(value || 0);
  const usd = Number.isFinite(n) && n > 0 ? n : 0;
  const digits = usd > 0 && usd < 0.01 ? 4 : 2;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: digits, maximumFractionDigits: digits }).format(usd);
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
    cost_usd: a.cost_usd + ((b.cost_usd ?? b.actual_cost_usd ?? b.estimated_cost_usd) || 0),
    unpriced_tokens: a.unpriced_tokens + (b.unpriced_tokens || 0),
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
  return { sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, estimated_cost_usd: 0, actual_cost_usd: 0, cost_usd: 0, unpriced_tokens: 0, total_tokens: 0, cache_hit_rate: 0, avg_tokens_per_session: 0 };
}

export function metricValue(bucket: UsageDay | UsageHour, metric: UsageMetric): number {
  return Number(bucket?.totals?.[metric] || 0);
}

export function modelDailyMetricValues(model: UsageModel, days: UsageDay[], metric: UsageMetric): number[] {
  return days.map((day) => {
    const modelDay = (model.daily || []).find((item) => item.date === day.date);
    return modelDay ? metricValue(modelDay, metric) : 0;
  });
}

export function modelHourlyMetricValues(model: UsageModel, hours: UsageHour[], metric: UsageMetric): number[] {
  return hours.map((hour) => {
    const modelHour = (model.hourly || []).find((item) => item.hour === hour.hour);
    return modelHour ? metricValue(modelHour, metric) : 0;
  });
}

export function periodSources(periods: UsagePeriod[], sources: UsageSource[], period: number): UsageSource[] {
  return periods.find((item) => item.days === period)?.sources || sources;
}

export type ChartPadding = number | { top: number; right: number; bottom: number; left: number };

function normalizeChartPadding(pad: ChartPadding) {
  return typeof pad === 'number' ? { top: pad, right: pad, bottom: pad, left: pad } : pad;
}

function chartMax(values: number[], maxValue?: number): number {
  return Math.max(1, Number(maxValue || 0), ...values.map((value) => Number(value || 0)));
}

export function chartPoint(index: number, value: number, count: number, width: number, height: number, pad: ChartPadding = 12, maxValue?: number) {
  const p = normalizeChartPadding(pad);
  const max = chartMax([value], maxValue);
  const innerW = Math.max(1, width - p.left - p.right);
  const innerH = Math.max(1, height - p.top - p.bottom);
  const x = p.left + (count <= 1 ? innerW / 2 : (innerW * index) / (count - 1));
  const y = p.top + innerH - (Math.max(0, value) / max) * innerH;
  return { x, y };
}

export function chartYAxisTicks(values: number[], count = 4, format: (value: number) => string = fmtTokens) {
  const max = chartMax(values);
  const steps = Math.max(2, count) - 1;
  return Array.from({ length: steps + 1 }, (_, index) => {
    const value = max - (max * index) / steps;
    return { value, label: format(value), pct: (index / steps) * 100 };
  });
}

export function formatMetricValue(metric: UsageMetric, value: number): string {
  return metric === 'cost_usd' ? fmtMoney(value) : fmtTokens(value);
}

export function chartTooltipLabel(model: string, dayLabel: string, value: number, metricLabel: string, displayValue = fmtTokens(value)): string {
  return `${model} · ${dayLabel} · ${metricLabel} ${displayValue}`;
}

export type ChartTooltipPlacement = 'above' | 'below';
export type ChartTooltipAlignment = 'start' | 'center' | 'end';

export function chartTooltipPlacement(pointY: number, chartHeight: number, topClearance = 44): ChartTooltipPlacement {
  const y = Number(pointY || 0);
  const height = Number(chartHeight || 0);
  return y < Math.min(topClearance, Math.max(0, height / 2)) ? 'below' : 'above';
}

export function chartTooltipAlignment(pointX: number, chartWidth: number, edgeClearance = 96): ChartTooltipAlignment {
  const x = Number(pointX || 0);
  const width = Number(chartWidth || 0);
  if (x < edgeClearance) return 'start';
  if (x > Math.max(edgeClearance, width - edgeClearance)) return 'end';
  return 'center';
}

type ChartPoint = ReturnType<typeof chartPoint>;

function smoothPointPath(points: ChartPoint[], firstCommand: 'M' | 'L' = 'M'): string {
  if (!points.length) return '';
  if (points.length === 1) return `${firstCommand} ${points[0].x.toFixed(2)} ${points[0].y.toFixed(2)}`;
  if (points.length === 2) return points.map((point, index) => `${index === 0 ? firstCommand : 'L'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`).join(' ');
  const [first, ...rest] = points;
  let path = `${firstCommand} ${first.x.toFixed(2)} ${first.y.toFixed(2)}`;
  for (let index = 0; index < rest.length; index += 1) {
    const start = points[index];
    const end = points[index + 1];
    const dx = (end.x - start.x) / 3;
    path += ` C ${(start.x + dx).toFixed(2)} ${start.y.toFixed(2)}, ${(end.x - dx).toFixed(2)} ${end.y.toFixed(2)}, ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
  }
  return path;
}

export function linePath(values: number[], width: number, height: number, pad: ChartPadding = 12, maxValue?: number): string {
  if (!values.length) return '';
  const points = values.map((value, index) => chartPoint(index, value, values.length, width, height, pad, maxValue || chartMax(values)));
  return smoothPointPath(points);
}

export function areaPath(values: number[], width: number, height: number, pad: ChartPadding = 12, maxValue?: number): string {
  const p = normalizeChartPadding(pad);
  const path = linePath(values, width, height, pad, maxValue);
  if (!path) return '';
  const baseY = height - p.bottom;
  const lastX = width - p.right;
  return `${path} L ${lastX.toFixed(2)} ${baseY.toFixed(2)} L ${p.left.toFixed(2)} ${baseY.toFixed(2)} Z`;
}

export function stackedAreaPath(lowerValues: number[], upperValues: number[], width: number, height: number, pad: ChartPadding = 12, maxValue?: number): string {
  if (!lowerValues.length || !upperValues.length) return '';
  const count = Math.min(lowerValues.length, upperValues.length);
  const max = maxValue || chartMax(upperValues);
  const upperPoints = upperValues.slice(0, count).map((value, index) => chartPoint(index, value, count, width, height, pad, max));
  const lowerPoints = lowerValues.slice(0, count).map((value, index) => chartPoint(index, value, count, width, height, pad, max)).reverse();
  return `${smoothPointPath(upperPoints)} ${smoothPointPath(lowerPoints, 'L')} Z`;
}
