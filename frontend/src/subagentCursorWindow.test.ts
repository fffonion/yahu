import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { latestSubagentWindowStart, shouldLoadHistoricalSubagentWindow, type SubagentProgress } from './subagentProgress';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const card = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

const row = (startedAt: number): SubagentProgress => ({
  sessionId: `sa-${startedAt}`,
  parentSessionId: 'parent',
  ancestryOmitted: false,
  task: `task-${startedAt}`,
  status: 'completed',
  startedAt,
  endedAt: startedAt + 10,
  messageCount: 1,
  toolCount: 0,
  apiCalls: 0,
  todos: [],
  activity: [],
});

describe('cursor subagent window', () => {
  test('uses the earliest start among the first bar latest ten as its coverage boundary', () => {
    const rows = Array.from({ length: 12 }, (_, index) => row(120 - index * 10));
    expect(latestSubagentWindowStart(rows)).toBe(30);
  });

  test('does not load a second window while the cursor is covered by the latest ten', () => {
    expect(shouldLoadHistoricalSubagentWindow(undefined, 30)).toBe(false);
    expect(shouldLoadHistoricalSubagentWindow(null, 30)).toBe(false);
    expect(shouldLoadHistoricalSubagentWindow(30, 30)).toBe(false);
    expect(shouldLoadHistoricalSubagentWindow(45, 30)).toBe(false);
    expect(shouldLoadHistoricalSubagentWindow(29, 30)).toBe(true);
  });

  test('renders the latest bar first in data ownership while placing the cursor bar between Goal and latest', () => {
    expect(app()).toContain("import { SubagentProgressStack } from './SubagentProgressCard';");
    expect(app()).toContain('<SubagentProgressStack sessionId={props.activeSessionId} beforeTime={subagentBeforeTime}');
    expect(card()).toContain('beforeTime={undefined}');
    expect(card()).toContain('shouldLoadHistoricalSubagentWindow');
    expect(card()).toContain('showGoal={false}');
  });

  test('assigns three semantic status-bar tones across every theme', () => {
    const styles = css();
    expect(styles).toContain('--subagent-goal-accent:var(--accent-2)');
    expect(styles).toContain('--subagent-card-accent:var(--accent)');
    expect(styles).toContain('--subagent-card-accent:var(--green)');
    expect(styles).toContain('.subagent-card-latest');
    expect(styles).toContain('.subagent-card-cursor');
  });
});
