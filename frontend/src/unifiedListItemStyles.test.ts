import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('unified Goal and subagent list items', () => {
  test('marks Goal and subagent rows with the shared item class', () => {
    const code = source();
    expect(code).toContain('subagent-list-item');
    expect(code).toContain('subagent-progress-node subagent-list-item');
    expect(code).toContain('subagent-goal-subgoals');
    expect(code).toContain('subagent-goal-milestones');
  });

  test('uses borderless full-width alternating item surfaces without a left rail', () => {
    const css = styles();
    expect(css).toContain('.subagent-list-item{');
    expect(css).toContain('width:100%');
    expect(css).toContain('border:0');
    expect(css).toContain('.subagent-list-item:nth-child(odd)');
    expect(css).toContain('.subagent-list-item:nth-child(even)');
    expect(css).toContain('.subagent-goal-milestones li{');
    expect(css).toContain('border-left:0');
    expect(css).toContain('.subagent-progress-node>details{width:100%;border:0');
  });
});
