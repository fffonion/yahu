import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('semantic corner radius hierarchy', () => {
  test('uses four radii from tiny controls to large panels', () => {
    expect(css()).toContain(':root{--radius-sm:4px;--radius-md:8px;--radius-card:14px;--radius-lg:18px}');
  });

  test('rounds visible panels and cards according to their scale', () => {
    const styles = css();
    expect(styles).toContain('.memory-grid label{border-radius:var(--radius-lg);overflow:hidden}');
    expect(styles).toContain('.subagent-progress-card{');
    expect(styles).toContain('border-radius:var(--radius-card);background:linear-gradient');
    expect(styles).toContain('.subagent-progress-node>details{border:1px solid');
    expect(styles).toContain('border-radius:var(--radius-md);background:color-mix');
  });

  test('uses smaller radii for compact summaries and row actions', () => {
    const styles = css();
    expect(styles).toContain('.turn-detail-group{display:grid;width:100%;max-width:920px;align-self:flex-start;margin:2px 0;border:0;border-radius:var(--radius-md);background:transparent;overflow:visible}');
    expect(styles).toContain('.turn-detail-summary{border-radius:var(--radius-md)}');
    expect(styles).toContain('.special-context-summary{');
    expect(styles).toContain('border-radius:var(--radius-md);color:var(--muted)');
    expect(styles).toContain('.workspace-tree-row>button{border-radius:var(--radius-sm)}');
  });

  test('applies semantic radii to legacy hard-coded controls and cards', () => {
    const styles = css();
    expect(styles).toContain('.pin-hit,.workspace-head button,.preview-head-actions .icon-btn,.subagent-progress-mark,.subagent-status-icon,.subagent-todo-box,.followup-drag-handle,.followup-action,.source-channel-chip,.skill-version{border-radius:var(--radius-sm)}');
    expect(styles).toContain('select,input,textarea,.rail-btn,.new-chat-btn,.session-item,.files-chip,.icon-btn,.send-btn,.load-history,.admin-form button,.job-card button,.settings-content button,.save-memory,.project-link,.dropdown-trigger,.dropdown-search,.dropdown-menu button,.file-row,.workspace-expand,.model-usage-row,.followup-item,.subagent-progress-stop,.mobile-terminal-special-keys button,.mobile-bottom-nav .rail-btn{border-radius:var(--radius-md)}');
    expect(styles).toContain('.admin-note,.job-card,.session-context-menu,.skill-context-menu,.skill-file-context-menu,.workspace-context-menu,.image-context-menu,.dropdown-menu,.usage-chart,.usage-chart-loading,.subagent-progress-card,.subagent-goal-panel,.user-minimap-popup{border-radius:var(--radius-card)}');
    expect(styles).toContain('.memory-grid label,.cron-list-pane,.cron-editor-pane,.insight-card,.insights-chart-card,.insights-panel,.web-terminal-host,.image-modal .modalbar{border-radius:var(--radius-lg)}');
  });

  test('keeps mobile composer and compact content surfaces rounded', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-box{width:100%;border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow);background:var(--surface)}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-box textarea{display:block;width:100%;height:48px;min-height:48px;max-height:20dvh;padding:12px 14px;border:0;border-radius:var(--radius-lg) var(--radius-lg) 0 0;');
    expect(styles).toContain('.msg-body blockquote{margin:8px 0;padding:6px 10px;border-left:3px solid var(--accent);background:color-mix(in srgb,var(--accent-soft) 46%,transparent);border-radius:var(--radius-md);');
    expect(styles).toContain('.workspace-markdown-preview.compact{max-height:260px;border:1px solid var(--border);border-radius:var(--radius-md);');
    expect(styles).toContain('.workspace-hex-viewer.compact{max-height:260px;border:1px solid var(--border);border-radius:var(--radius-md);');
  });
});
