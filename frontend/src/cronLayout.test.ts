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
    expect(styles).toContain('.cron-field.cron-prompt,.cron-field.cron-script{grid-column:1/-1}');
    expect(styles).toContain('.cron-main .cron-prompt{min-height:0;height:100%;grid-template-rows:auto minmax(0,1fr)}');
    expect(styles).toContain('.cron-main .cron-prompt textarea{min-height:0;height:100%;resize:vertical}');
    expect(styles).toContain('.cron-script textarea{min-height:96px;height:96px');
  });

  test('header exposes save run and delete actions with standard header icon buttons', () => {
    const source = app();
    expect(source).toContain('className="header-actions cron-header-actions"');
    expect(source).toContain('className="icon-btn cron-action-btn"');
    expect(source).toContain('className="icon-btn cron-action-btn danger"');
    expect(source).toContain("aria-label={t('cron.saveAria')}");
    expect(source).toContain("aria-label={t('cron.runAria')}");
    expect(source).toContain("aria-label={t('cron.deleteAria')}");
    expect(source).not.toContain('className="cron-detail-actions"');
    expect(source).not.toContain('aria-label={paused ? \'resume\' : \'pause\'}');
  });

  test('delete action requires a dangerous confirmation dialog before calling the delete API', () => {
    const source = app();
    const deleteStart = source.indexOf('const deleteCronJob = useCallback(async () => {');
    const deleteEnd = source.indexOf('useEffect(() => { loadModels();', deleteStart);
    const deleteBlock = source.slice(deleteStart, deleteEnd);
    expect(deleteBlock).toContain("requestConfirm(t('cron.deleteTitle'), tf('cron.deleteConfirm', cronName || cronEditingId), true)");
    expect(deleteBlock.indexOf("requestConfirm(t('cron.deleteTitle')")).toBeLessThan(deleteBlock.indexOf("method: 'DELETE'"));
    expect(source).toContain("'cron.deleteTitle'");
    expect(source).toContain("'cron.deleteConfirm'");
  });

  test('loads latest cron output through the Hermes API server proxy', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("/api/jobs/${encodeURIComponent(id)}/output/latest");
    expect(source).toContain('className="cron-output-panel cron-fullwidth"');
    expect(source).toContain("{t('cron.lastOutput')}");
    expect(styles).toContain('.cron-output-panel{display:grid;gap:7px;min-width:0}');
    expect(styles).toContain('.cron-output-panel pre{margin:0;overflow:visible;white-space:pre-wrap;word-break:break-word;font:12px/1.45 var(--mono);color:var(--text);background:var(--surface-2);border:1px solid var(--border);border-radius:var(--radius-md);padding:10px 12px}');
    expect(styles).not.toContain('.cron-output-panel{display:grid;gap:8px;border:1px solid var(--border);');
    expect(styles).not.toContain('.cron-output-panel pre{max-height:240px');
    expect(styles).not.toContain('.cron-output-panel pre{margin:0;overflow:visible;white-space:pre-wrap;word-break:break-word;font:12px/1.45 var(--mono);color:var(--text);background:var(--surface);');
  });

  test('places the latest output timestamp first and lets the page carry output scrolling', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="cron-output-title"');
    expect(source).toContain('className="cron-output-timestamp"');
    expect(source.indexOf('className="cron-output-timestamp"')).toBeLessThan(source.indexOf("{t('cron.lastOutput')}"));
    expect(source).not.toContain('{props.cronOutput?.timestamp && <small>{props.cronOutput.timestamp}</small>}');
    expect(styles).toContain('.cron-detail-wrap{min-height:0;overflow:auto;padding:22px}');
    expect(styles).toContain('.cron-main .cron-detail{height:auto;min-height:100%;grid-template-rows:auto minmax(460px,1fr) auto auto;overflow:visible}');
  });

  test('mobile cron editor adapts prompt height to the viewport instead of using fixed rows', () => {
    const styles = css();
    expect(styles).toContain('.cron-main .cron-detail{height:auto;min-height:100%;grid-template-columns:1fr;grid-template-rows:auto auto minmax(96px,1fr) auto auto auto;align-content:stretch;overflow:visible}');
    expect(styles).toContain('.cron-main .cron-prompt{min-height:0;height:100%;grid-template-rows:auto minmax(0,1fr)}');
    expect(styles).toContain('.cron-main .cron-prompt textarea{min-height:96px;height:100%;resize:none}');
    expect(styles).toContain('.cron-main .cron-script textarea{min-height:64px;height:64px}');
    expect(styles).toContain('.cron-detail-wrap{height:100%;min-height:0;overflow:auto;padding:8px 10px calc(var(--mobile-bottom-nav-height) + 10px + env(safe-area-inset-bottom,0px))}');
  });
});
