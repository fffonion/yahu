import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('cron manager split editor layout', () => {
  test('renders cron jobs in the left sidebar, mirroring the session list layout', () => {
    const source = app();
    expect(source).toContain("mode === 'cron' ? <CronSidebar");
    expect(source).toContain('className="cron-sidebar-list"');
    expect(source).toContain('className={`cron-sidebar-row ${jobId(j) === editingId ? \'active\' : \'\'}`}');
    expect(source).toContain('onClick={() => { beginCronEdit(j); closeMobileSidebar(); }}');
    expect(source).not.toContain('className="admin-content cron-layout"');
  });

  test('right pane is a detail editor with a large prompt and full-width script', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function CronMain');
    expect(source).toContain('className="main-panel cron-main"');
    expect(source).toContain('className="cron-detail"');
    expect(source).toContain('className="cron-field cron-prompt"');
    expect(source).toContain('className="cron-field cron-script"');
    expect(styles).toContain('.cron-field.cron-prompt,.cron-field.cron-script,.cron-detail-actions{grid-column:1/-1}');
    expect(styles).toContain('.cron-main .cron-prompt{min-height:0;height:100%;grid-template-rows:auto minmax(0,1fr)}');
    expect(styles).toContain('.cron-main .cron-prompt textarea{min-height:0;height:100%;resize:vertical}');
    expect(styles).toContain('.cron-script textarea{min-height:96px;height:96px');
  });

  test('right detail pane exposes save run and delete actions together', () => {
    const source = app();
    expect(source).toContain('className="cron-detail-actions"');
    expect(source).toContain("aria-label={t('cron.saveAria')}");
    expect(source).toContain("aria-label={t('cron.runAria')}");
    expect(source).toContain("aria-label={t('cron.deleteAria')}");
    expect(source).not.toContain('aria-label={paused ? \'resume\' : \'pause\'}');
  });

  test('mobile cron editor adapts prompt height to the viewport instead of using fixed rows', () => {
    const styles = css();
    expect(styles).toContain('.cron-main .cron-detail{height:100%;min-height:0;grid-template-columns:1fr;grid-template-rows:auto auto minmax(96px,1fr) auto auto auto;align-content:stretch;overflow:auto}');
    expect(styles).toContain('.cron-main .cron-prompt{min-height:0;height:100%;grid-template-rows:auto minmax(0,1fr)}');
    expect(styles).toContain('.cron-main .cron-prompt textarea{min-height:96px;height:100%;resize:none}');
    expect(styles).toContain('.cron-main .cron-script textarea{min-height:64px;height:64px}');
    expect(styles).toContain('.cron-detail-wrap{height:100%;min-height:0;overflow:hidden;padding:8px 10px calc(var(--mobile-bottom-nav-height) + 10px + env(safe-area-inset-bottom,0px))}');
  });
});
