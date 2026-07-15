import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('completed subagent list spacing', () => {
  test('expanded recent tree packs completed rows at their content height', () => {
    const treeRule = css().match(/\.subagent-progress-tree\{([^}]*)\}/)?.[1] || '';
    expect(treeRule).toContain('display:grid');
    expect(treeRule).toContain('align-content:start');
    expect(treeRule).toContain('gap:7px');
  });
});
