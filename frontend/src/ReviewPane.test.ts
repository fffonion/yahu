import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const pane = () => readFileSync(new URL('./ReviewPane.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('review pane data flow', () => {
  test('lists diffs first and falls back to the raw file text', () => {
    const source = pane();
    expect(source).toContain('/workspace/diff?path=');
    expect(source).toContain('/workspace/file?path=');
    expect(source).toContain('encodeURIComponent(path)');
    expect(source).toContain('const token = requestRef.current + 1;');
    expect(source).toContain('if (token !== requestRef.current) return;');
  });

  test('surfaces untracked files and merge-base diffs instead of a bare no-diff notice', () => {
    const source = pane();
    expect(source).toContain('untracked?: boolean;');
    expect(source).toContain('base?: string | null;');
    expect(source).toContain('untracked: Boolean(payload.untracked)');
    expect(source).toContain("if (view.untracked) notices.push(t('review.untracked'));");
    expect(source).toContain("if (view.hasDiff && view.base) notices.push(tf('review.baseDiff', view.base));");
    expect(source).toContain("if (!view.hasDiff && view.base) notices.push(tf('review.noChangesInRange', view.base));");
    expect(source).toContain('!view.hasDiff && !view.untracked && !view.base');
  });

  test('offers a compare-range menu with commits and a branch base', () => {
    const source = pane();
    expect(source).toContain('/workspace/commits?path=');
    expect(source).toContain("const baseParam = rangeKey === 'auto' ? '' : `&base=${encodeURIComponent(rangeKey)}`;");
    expect(source).toContain("type ReviewRange = 'auto' | 'branch' | `commits:${number}`;");
    expect(source).toContain("tf('review.rangeBranch', rangeInfo?.base || 'main')");
    expect(source).toContain("pickRange('commits:1')");
    expect(source).toContain("pickRange('branch')");
    expect(source).toContain('const nextKey = `commits:${index + 2}` as ReviewRange;');
    expect(source).toContain("t('review.rangeSingle')");
    expect(source).toContain('{commit.short} · {commit.date}');
    expect(source).toContain('}, [path, rangeKey]);');
  });

  test('loads older compare ranges on demand and labels an empty selected range', () => {
    const source = pane();
    expect(source).toContain('skip=${rangeCommits.length}');
    expect(source).toContain('setRangeInfo((current) =>');
    expect(source).toContain("t('review.loadOlder')");
    expect(source).toContain("tf('review.noChangesInRange', view.base)");
  });

  test('reuses the shared diff parsers instead of re-implementing hunk math', () => {
    const source = pane();
    expect(source).toContain("from './reviewDiff'");
    expect(source).toContain("from './reviewComments'");
    expect(source).toContain("from './reviewReport'");
    expect(source).toContain('parseUnifiedDiff(diffText, path)');
    expect(source).toContain('buildSplitRows(lines)');
    expect(source).toContain('numberedFileLines(content, path)');
    expect(source).toContain('commentsFor(review, path)');
  });

  test('comments and the report live in session storage only', () => {
    const source = pane();
    expect(source).toContain('addComment(window.sessionStorage, path, draft.anchor, body)');
    expect(source).toContain('updateComment(window.sessionStorage, draft.commentId, body)');
    expect(source).toContain('deleteComment(window.sessionStorage, id)');
    expect(source).toContain('clearReview(window.sessionStorage);');
    expect(source).toContain('loadReview(window.sessionStorage)');
    expect(source).not.toContain('localStorage');
  });

  test('reads each commented current file before writing the report', () => {
    const source = pane();
    expect(source).toContain('new Set(current.comments.map((comment) => comment.path))');
    expect(source).toContain('/workspace/file?path=${encodeURIComponent(file)}');
    expect(source).toContain('const contents = new Map(');
    expect(source.indexOf('const contents = new Map(')).toBeLessThan(source.indexOf("fetch('/workspace/review'"));
  });

  test('submitting posts the built report and copies the follow-up prompt', () => {
    const source = pane();
    expect(source).toContain("fetch('/workspace/review'");
    expect(source).toContain("method: 'POST'");
    expect(source).toContain("'content-type': 'application/json'");
    expect(source).toContain('filename: reviewFileName(),');
    expect(source).toContain('buildReviewReport(current, { workspace: abs || window.location.origin, generatedAt: Date.now(), contents })');
    expect(source).toContain('navigator.clipboard.writeText(text)');
    expect(source).toContain("document.execCommand('copy')");
    expect(source).toContain('typeof payload.abs === \'string\'');
    expect(source).toContain('payload.abs.length > 0 ? payload.abs : window.location.origin');
  });
});

describe('review pane rendering and interaction', () => {
  test('renders unified and split tables with testable line anchors', () => {
    const source = pane();
    expect(source).toContain('review-unified-table');
    expect(source).toContain('review-split-table');
    expect(source).toContain('data-line-new={line.newLine}');
    expect(source).toContain('data-line-old={line.oldLine}');
    expect(source).toContain('data-line-old={left?.oldLine}');
    expect(source).toContain('data-line-new={right?.newLine}');
    expect(source).toContain('dangerouslySetInnerHTML');
    expect(source).toContain('newLineAnchors(lines)');
  });

  test('both layouts attach every comment button to a new-file line', () => {
    const source = pane();
    expect(source).toContain('const newAnchors = useMemo(() => newLineAnchors(lines), [lines]);');
    expect(source).toContain("const anchor: ReviewAnchor = { side: 'new', line: number };");
    expect(source).toContain("gutterCell(line, 'old', false");
    expect(source).toContain("gutterCell(left, 'old', false");
    expect(source).toContain('newAnchors.get(line)');
    expect(source).toContain('newAnchors.get(right ?? left!)');
  });

  test('a deleted run mapped to one new line shows its comment only once per layout', () => {
    const source = pane();
    expect(source).toContain('const unifiedDisplayed = new Set<string>();');
    expect(source).toContain('const splitDisplayed = new Set<string>();');
    expect(source).toContain('displayed.has(itemKey)');
    expect(source).toContain('draft.rowKey === key');
  });

  test('every source gutter carries the hover add-comment control and inline editor', () => {
    const source = pane();
    expect(source).toContain('className="review-add-comment"');
    expect(source).toContain("aria-label={t('review.addComment')}");
    expect(source).toContain('className="review-comment-input"');
    expect(source).toContain('className="review-thread"');
    expect(source).toContain('className="review-comment-card"');
    expect(source).toContain('className="review-comment-edit"');
    expect(source).toContain('className="review-comment-delete"');
    expect(source).toContain("t('review.edit')");
    expect(source).toContain("t('review.delete')");
  });

  test('the editor cancels on Escape and saves on Cmd or Ctrl plus Enter', () => {
    const source = pane();
    expect(source).toContain("event.key === 'Escape'");
    expect(source).toContain("event.key === 'Enter' && (event.metaKey || event.ctrlKey)");
    expect(source).toContain('const trimmed = body.trim();');
    expect(source).toContain('if (!trimmed) return;');
  });

  test('the toolbar exposes chips, the view switch, submit and exit', () => {
    const source = pane();
    expect(source).toContain('className="review-toolbar"');
    expect(source).toContain("review-file-chip${file === path ? ' active' : ''}");
    expect(source).toContain('onSelectFile(file)');
    expect(source).toContain('onRemoveFile(file)');
    expect(source).toContain('className="review-submit"');
    expect(source).toContain('className="review-exit-button"');
    expect(source).toContain("aria-label={t('review.exit')}");
    expect(source).toContain('onClick={onExit}');
    expect(source).toContain('className="review-copy-prompt"');
  });

  test('narrow screens force the unified view and drop the split control', () => {
    const source = pane();
    expect(source).toContain("const COMPACT_QUERY = '(max-width: 760px)';");
    expect(source).toContain('window.matchMedia(COMPACT_QUERY)');
    expect(source).toContain("query.addEventListener('change', sync)");
    expect(source).toContain("return () => query.removeEventListener('change', sync);");
    expect(source).toContain("const effectiveMode: ReviewMode = narrow ? 'unified' : mode;");
    expect(source).toContain('disabled={narrow}');
  });

  test('exports the typed component contract', () => {
    const source = pane();
    expect(source).toContain('export type ReviewPaneProps = {');
    expect(source).toContain('onSelectFile: (path: string) => void;');
    expect(source).toContain('onRemoveFile: (path: string) => void;');
    expect(source).toContain('onExit: () => void;');
    expect(source).toContain('export default ReviewPane;');
  });
});

describe('review pane copy', () => {
  test('uses the shared review translations', () => {
    const source = pane();
    for (const key of [
      'workspace.review',
      'review.unified',
      'review.split',
      'review.addComment',
      'review.commentPlaceholder',
      'review.save',
      'review.cancel',
      'review.edit',
      'review.delete',
      'review.submit',
      'review.submitted',
      'review.submitFailed',
      'review.copyPrompt',
      'review.copied',
      'review.noDiff',
      'review.files',
      'review.pending',
      'review.exit',
      'review.loading',
      'review.truncated',
    ]) {
      expect(source).toContain(`'${key}'`);
    }
    expect(source).toContain("tf('workspace.previewFailed', view.message)");
  });
});

describe('review pane styles', () => {
  test('adds diff tokens and keeps the earlier theme rules intact', () => {
    const css = styles();
    expect(css).toContain('--diff-add:rgba(46,160,67,.18)');
    expect(css).toContain('--diff-add-strong:#2ea043');
    expect(css).toContain('--diff-del:rgba(248,81,73,.16)');
    expect(css).toContain('--diff-del-strong:#f85149');
    expect(css).toContain('--diff-gutter-bg:color-mix(in srgb,var(--surface-2) 80%,transparent)');
    expect(css).toContain(':root{--bg:#f4f1ea');
    expect(css.indexOf('.review-pane{')).toBeGreaterThan(css.indexOf('.app-shell{'));
  });

  test('styles the pane grid, gutters, rows, threads and toolbar', () => {
    const css = styles();
    for (const selector of [
      '.review-pane{',
      '.review-diff-table{',
      '.review-gutter{',
      '.review-add-comment{',
      '.review-row-add{',
      '.review-row-del{',
      '.review-thread{',
      '.review-comment-card{',
      '.review-comment-input{',
      '.review-toolbar{',
      '.review-file-chip{',
      '.review-submit{',
      '.review-submit-result{',
    ]) {
      expect(css).toContain(selector);
    }
    expect(css).toContain('font:13px/1.55 ui-monospace');
    expect(css).toContain('user-select:none');
    expect(css).toContain('opacity:0');
    expect(css).toContain('.review-gutter:hover .review-add-comment,.review-add-comment:hover');
    expect(css).toContain('border-left:3px solid var(--diff-add-strong)');
    expect(css).toContain('border-left:3px solid var(--diff-del-strong)');
    expect(css).toContain('resize:vertical');
    expect(css).toContain('position:sticky');
  });

  test('mobile review shows a compact single-row toolbar and uses its file chip as the only file header', () => {
    const css = styles();
    const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
    expect(css).toContain('@media (max-width:760px)');
    expect(css).toContain('.workspace-editor-preview.is-review{grid-template-rows:minmax(0,1fr)}');
    expect(css).toContain('.workspace-editor-preview.is-review .preview-head{display:none}');
    expect(app).toContain("review?.active && preview.kind === 'text' ? 'workspace-editor-preview is-review' : 'workspace-editor-preview'");
    expect(css).toContain('.review-toolbar{flex-wrap:nowrap;gap:4px;padding:4px 8px}');
    expect(css).toContain('.review-file-chips{flex:1 1 0;flex-wrap:nowrap;overflow-x:auto');
    expect(css).toContain('.review-file-chip{flex:0 0 auto;min-height:28px;');
    expect(pane()).toContain("{file.split('/').slice(-2).join('/')}");
    expect(css).toContain('.review-file-name{display:none}');
    expect(css).toContain('.review-file-path{display:none}');
    expect(css).toContain('.review-range{position:static;flex:0 0 30px}');
    expect(css).toContain('.review-range-trigger>span,.review-range-trigger svg:last-child{display:none}');
    expect(css).toContain('.review-range-menu{position:absolute;top:calc(100% + 4px);left:8px;right:8px;width:auto');
    expect(css).toContain('.review-toolbar-actions{flex:0 0 auto;margin-left:0;gap:4px}');
    expect(pane()).toContain("aria-label={t('review.range')}");
    expect(css).toContain('.review-split-table{display:none}');
    expect(css).toContain('.review-view-toggle{display:none}');
    expect(css).toContain('.review-diff-body{padding:8px;overflow-x:auto;overscroll-behavior-x:contain');
    expect(css).toContain('.review-diff-table{table-layout:auto;width:max-content;min-width:100%}');
    expect(css).toContain('.review-code-cell{white-space:pre;overflow-wrap:normal');
    expect(css).toContain('.review-gutter{padding:0 5px 0 28px;font-size:11px}');
  });
});
