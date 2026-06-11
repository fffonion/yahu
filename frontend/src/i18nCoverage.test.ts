import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

function section(source: string, start: string, end: string) {
  return source.slice(source.indexOf(start), source.indexOf(end));
}

describe('page i18n coverage', () => {
  test('Insights, Workspace, and Cron UI copy uses translation keys instead of fixed English text', () => {
    const source = app();
    const targets = [
      section(source, 'function InsightsMain', 'function ChatSidebar'),
      section(source, 'function WorkspaceAside', 'function AdminMain'),
      section(source, 'function CronSidebar', 'function MemoryPanel'),
      section(source, 'function CustomDialog', 'function ModeSidebar'),
      section(source, 'function ModeSidebar', 'function InsightsMain'),
    ].join('\n');

    for (const text of [
      'Loading usage…',
      'Usage insights unavailable',
      'Refresh usage',
      'Usage controls',
      'Top model',
      'Other signals',
      'Show stacked chart',
      'Show unstacked chart',
      'No model usage for this metric.',
      'Rename item',
      'Choose a new file or folder name.',
      'Delete workspace item',
      'Workspace item is not editable',
      'Close preview',
      'Cancel edit',
      'Cron jobs</h2>',
      'scheduled jobs</p>',
      '<span>Name</span>',
      'placeholder="Job name"',
      'origin (reply to chat)',
      'all connected channels',
      'Appearance',
      '<span>Theme</span>',
      '>Confirm<',
    ]) {
      expect(targets).not.toContain(text);
    }

    for (const key of [
      'insights.loadingUsage',
      'insights.unavailable',
      'insights.refreshUsage',
      'insights.otherSignals',
      'insights.showStackedChart',
      'workspace.renameMessage',
      'workspace.deleteTitle',
      'workspace.itemNotEditable',
      'cron.deliverOrigin',
      'cron.deliverAll',
      'theme.appearance',
      'dialog.confirm',
    ]) {
      expect(source).toContain(`t('${key}')`);
    }
  });

  test('new i18n entries cover every configured language', () => {
    const source = i18n();
    for (const key of [
      'insights.title',
      'insights.unavailable',
      'insights.metric.total_tokens',
      'insights.noMetricUsage',
      'workspace.renameMessage',
      'workspace.deleteConfirm',
      'cron.deliverOrigin',
      'cron.deliverAll',
      'theme.appearance',
      'dialog.confirm',
    ]) {
      const line = source.split('\n').find((item) => item.includes(`'${key}'`)) || '';
      expect(line).toContain('en:');
      expect(line).toContain("'zh-CN':");
      expect(line).toContain("'zh-TW':");
      expect(line).toContain('ja:');
    }
  });
});
