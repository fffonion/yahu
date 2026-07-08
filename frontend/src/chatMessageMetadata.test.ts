import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('final assistant message metadata', () => {
  test('normalizes per-turn time token and price fields from API payloads', () => {
    const source = app();
    expect(source).toContain('type ChatTurnMetrics = { elapsedMs?: number; inputTokens?: number; outputTokens?: number; totalTokens?: number; costUsd?: number };');
    expect(source).toContain('turnMetrics?: ChatTurnMetrics');
    expect(source).toContain('function readTurnMetrics(raw: any): ChatTurnMetrics | undefined');
    expect(source).toContain("numericMetric(raw, ['duration_ms', 'elapsed_ms', 'latency_ms', 'time_ms'])");
    expect(source).toContain("numericMetric(usage, ['input_tokens', 'prompt_tokens'])");
    expect(source).toContain("numericMetric(usage, ['output_tokens', 'completion_tokens'])");
    expect(source).toContain("numericMetric(usage, ['total_tokens', 'tokens', 'token_count'])");
    expect(source).toContain("numericMetric(usage, ['cost_usd', 'estimated_cost_usd', 'actual_cost_usd', 'cost'])");
    expect(source).toContain('const metrics = readTurnMetrics(raw);');
    expect(source).toContain('if (metrics) msg.turnMetrics = metrics;');
  });

  test('renders final assistant metadata from backend-provided metrics only', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function messageTurnMetadata(message: ChatMessage): string');
    expect(source).toContain('const turnMetadata = messageTurnMetadata(message);');
    expect(source).toContain("message.role === 'assistant' && !isPending && !isToolPrelude");
    expect(source).toContain('<div className="msg-turn-metadata" aria-label={t(\'chat.details\')}>{turnMetadata}</div>');
    expect(source).not.toContain('numericTimestampMs(turnStartedAt)');
    expect(source).not.toContain('endMs - startMs');
    expect(styles).toContain('.msg-turn-metadata{margin-top:10px;color:color-mix(in srgb,var(--muted) 82%,transparent);font-size:11px;line-height:1.35}');
  });

  test('formats subsecond elapsed time as under one second, never 0ms', () => {
    const source = app();
    expect(source).toContain("if (ms < 1000) return 'time <1s';");
    expect(source).not.toContain('Math.round(ms)}ms');
  });

  test('omits unavailable token and cost metadata instead of rendering dash placeholders', () => {
    const source = app();
    expect(source).not.toContain("return 'tokens —';");
    expect(source).not.toContain("return 'cost —';");
    expect(source).toContain('const metadataParts = [formatTurnDuration(elapsedMs)];');
    expect(source).toContain('if (totalTokens !== undefined) metadataParts.push(`${formatTurnTokenCount(totalTokens)}${detail}`);');
    expect(source).toContain('if (metrics.costUsd !== undefined) metadataParts.push(formatTurnCost(metrics.costUsd));');
    expect(source).toContain("return metadataParts.join(' · ');");
  });

  test('live streaming uses backend elapsed time and usage on the completed assistant row', () => {
    const source = app();
    expect(source).not.toContain('const turnStartedAtMs = Date.now();');
    expect(source).toContain('let turnMetrics: ChatTurnMetrics | undefined;');
    expect(source).toContain('turnMetrics = mergeTurnMetrics(turnMetrics, readTurnMetrics(payload));');
    expect(source).toContain('turnMetrics = mergeTurnMetrics(turnMetrics, readTurnMetrics(payload.messages?.[0]));');
    expect(source).not.toContain('elapsedMs: Date.now() - turnStartedAtMs');
    expect(source).toContain('turnMetrics: turnMetrics');
  });
});
