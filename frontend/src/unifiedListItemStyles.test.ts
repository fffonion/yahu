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

  test('uses one shared row rhythm for every Goal and subagent item type', () => {
    const css = styles();
    expect(css).toContain('.subagent-list-item{width:100%;box-sizing:border-box;border:0;border-radius:0;margin:0;padding:7px 10px;min-height:36px;');
    expect(css).toContain('.subagent-progress-node.subagent-list-item{padding:0;min-height:36px}');
    expect(css).toContain('.subagent-progress-node>details>summary{width:100%;box-sizing:border-box;min-height:36px;padding:7px 10px;margin:0}');
    expect(css).toContain('.subagent-progress-todos li,.subagent-goal-subgoals li,.subagent-goal-milestones li{padding:7px 10px;min-height:36px;margin:0;');
  });

  test('uses borderless full-width alternating item surfaces without a left rail', () => {
    const css = styles();
    expect(css).toContain('.subagent-list-item{');
    expect(css).toContain('width:100%');
    expect(css).toContain('border:0');
    expect(css).toContain('.subagent-list-item:nth-child(odd)');
    expect(css).toContain('.subagent-list-item:nth-child(even)');
    expect(css).toContain('.subagent-list-item:nth-child(odd){background:color-mix(in srgb,var(--surface-2) 34%,var(--surface))}');
    expect(css).toContain('.subagent-list-item:nth-child(even){background:color-mix(in srgb,var(--surface-2) 22%,var(--surface))}');
    expect(css).toContain('.subagent-goal-milestones li{');
    expect(css).toContain('border-left:0');
    expect(css).toContain('.subagent-progress-node>details{width:100%;border:0');
  });
});
