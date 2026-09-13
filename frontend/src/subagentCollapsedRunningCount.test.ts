import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const card = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('collapsed subagent running count', () => {
  test('shows the number of running subagents when more than one is active', () => {
    const source = card();
    expect(source).toContain("const runningCount = snapshot?.subagents.filter((item) => item.status === 'running').length || 0;");
    expect(source).toContain('const preview = previewSubagent(snapshot.subagents);');
    expect(source).toContain('<SubagentProgressPreview node={preview} runningCount={runningCount} nowSeconds={nowSeconds} />');
    expect(source).toContain("runningCount > 1 ? tf('subagents.runningCount', runningCount) : statusLabel(node.status)");
    expect(i18n()).toContain("'subagents.runningCount': { en: '{0} running', 'zh-CN': '{0} 运行中', 'zh-TW': '{0} 執行中', ja: '{0} 実行中' }");
  });
});
