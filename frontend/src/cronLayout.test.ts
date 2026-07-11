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

  test('header exposes save run pause/resume and delete actions with standard header icon buttons', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="header-actions cron-header-actions"');
    expect(source).toContain('className="icon-btn cron-action-btn"');
    expect(source).toContain('className="icon-btn cron-action-btn cron-pause-toggle"');
    expect(source).toContain('className="icon-btn cron-action-btn danger"');
    expect(source).toContain("aria-label={t('cron.saveAria')}");
    expect(source).toContain("aria-label={t('cron.runAria')}");
    expect(source).toContain("aria-label={t(paused ? 'cron.resumeAria' : 'cron.pauseAria')}");
    expect(source).toContain("title={t(paused ? 'cron.resume' : 'cron.pause')}");
    expect(source).toContain('{paused ? <Play /> : <Pause />}');
    expect(styles).toContain('.cron-pause-toggle svg{width:18px;height:18px}');
    expect(source).toContain("aria-label={t('cron.deleteAria')}");
    expect(source).not.toContain('className="cron-detail-actions"');
  });

  test('pause/resume action posts the selected lifecycle endpoint and refreshes jobs', () => {
    const source = app();
    const actionStart = source.indexOf('const toggleCronPaused = useCallback(async () => {');
    const actionEnd = source.indexOf('const deleteCronJob = useCallback(async () => {', actionStart);
    const actionBlock = source.slice(actionStart, actionEnd);
    expect(actionBlock).toContain("const action = paused ? 'resume' : 'pause';");
    expect(actionBlock).toContain("/api/jobs/${encodeURIComponent(cronEditingId)}/${action}");
    expect(actionBlock).toContain("method: 'POST'");
    expect(actionBlock).toContain('await loadCronJobs();');
    expect(source).toContain('toggleCronPaused={toggleCronPaused}');
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
    expect(styles).toContain('.cron-output-content{margin:0;overflow:visible;white-space:normal;word-break:break-word;font:13px/1.55 var(--font);color:var(--text);background:var(--surface-2);border:1px solid var(--border);border-radius:var(--radius-md);padding:10px 12px}');
    expect(styles).not.toContain('.cron-output-panel{display:grid;gap:8px;border:1px solid var(--border);');
    expect(styles).not.toContain('.cron-output-panel pre{max-height:240px');
    expect(styles).not.toContain('.cron-output-panel pre{margin:0;overflow:visible;white-space:pre-wrap;word-break:break-word;font:12px/1.45 var(--mono);color:var(--text);background:var(--surface);');
  });

  test('latest cron output renders markdown and image media with the chat lightbox', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const cronOutputText = cronOutputDisplayText(props.cronOutput, props.cronOutputLoading);');
    expect(source).toContain('dangerouslySetInnerHTML={{ __html: markdownText(cronOutputText) }}');
    expect(source).toContain('const cronLightboxImages = useMemo(() => chatMediaImagesFromMarkdown(cronOutputText)');
    expect(source).toContain('onClick={onCronOutputMediaClick}');
    expect(source).toContain('<ChatImageLightbox items={cronLightboxImages} current={cronImageModal}');
    expect(source).not.toContain('<pre>{props.cronOutputLoading ? t(\'cron.loadingOutput\')');
    expect(styles).toContain('.cron-output-panel .md-media-open{display:block;cursor:zoom-in}');
    expect(styles).toContain('.cron-output-panel .md-media img,.cron-output-panel .md-media video{width:100%;height:auto;max-width:100%;border-radius:12px;border:1px solid var(--border);background:var(--surface);display:block}');
  });

  test('selected cron detail shows the enabled toolsets from the API job row', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('enabled_toolsets?: string[]; enabledToolsets?: string[]');
    expect(source).toContain('function cronEnabledToolsets(job?: Job | null): string[]');
    expect(source).toContain('const activeCronJob = cronJobs.find((job) => jobId(job) === cronEditingId) || null;');
    expect(source).toContain('currentJob={activeCronJob}');
    expect(source).toContain('className="cron-tools-field cron-fullwidth"');
    expect(source).toContain("{t('cron.enabledTools')}");
    expect(source).toContain('cronEnabledToolsets(props.currentJob).map((toolset) => <span className="cron-tool-chip" key={toolset}>{toolset}</span>)');
    expect(source).toContain("{!cronEnabledToolsets(props.currentJob).length && <span className=\"cron-tool-chip muted\">{t('cron.allDefaultTools')}</span>}");
    expect(styles).toContain('.cron-tools-field{display:grid;gap:7px;min-width:0}');
    expect(styles).toContain('.cron-tool-list{display:flex;flex-wrap:wrap;gap:6px;min-width:0}');
    expect(styles).toContain('.cron-tool-chip{display:inline-flex;align-items:center;min-height:28px;padding:5px 9px;border:1px solid var(--border);border-radius:999px;background:var(--surface-2);color:var(--text);font-size:12px;line-height:1.2}');
  });

  test('selected cron detail shows pinned provider and model from the API job row', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('model?: string | { model?: string; provider?: string }; provider?: string; provider_snapshot?: string; model_snapshot?: string; no_agent?: boolean; noAgent?: boolean');
    expect(source).toContain('function cronPinnedModel(job?: Job | null)');
    expect(source).toContain('className="cron-model-field cron-fullwidth"');
    expect(source).toContain("{t('cron.pinnedModel')}");
    expect(source).toContain('cronPinnedModel(props.currentJob)');
    expect(styles).toContain('.cron-model-field{display:grid;gap:7px;min-width:0}');
    expect(styles).toContain('.cron-model-value{display:flex;flex-wrap:wrap;gap:6px;align-items:center;min-width:0}');
  });

  test('no-agent cron jobs show a non-agent label instead of model/provider chips', () => {
    const source = app();
    expect(source).toContain('if (job?.no_agent || job?.noAgent) return { nonAgent: true };');
    expect(source).toContain("pinnedModel.nonAgent ? <span className=\"cron-tool-chip muted\">{t('cron.nonAgentJob')}</span>");
    expect(source).toContain("'cron.nonAgentJob'");
  });

  test('delivery target is editable and saved in the cron patch body', () => {
    const source = app();
    expect(source).toContain('setDeliver: (v: string) => void');
    expect(source).toContain('buildCronPatch({ name: cronName, schedule: cronSchedule, prompt: cronPrompt, script: cronScript, deliver: cronDeliver })');
    expect(source).toContain('cronEditableValues(job)');
    expect(source).toContain('setCronDeliver(values.deliver)');
    expect(source).toContain('className="cron-field cron-fullwidth cron-deliver-field"');
    expect(source).toContain('value={props.deliver}');
    expect(source).toContain('onChange={(e) => props.setDeliver(e.target.value)}');
    expect(source).not.toContain('value={deliverDisplay(props.deliver)} readOnly');
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
