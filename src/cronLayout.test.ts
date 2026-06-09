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
    expect(styles).toContain('.cron-main .cron-prompt textarea{min-height:460px;height:auto}');
    expect(styles).toContain('.cron-script textarea{min-height:96px;height:96px');
  });

  test('right detail pane exposes save run and delete actions together', () => {
    const source = app();
    expect(source).toContain('className="cron-detail-actions"');
    expect(source).toContain('aria-label="save cron job"');
    expect(source).toContain('aria-label="run cron job"');
    expect(source).toContain('aria-label="delete cron job"');
    expect(source).not.toContain('aria-label={paused ? \'resume\' : \'pause\'}');
  });
});
