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

  test('submitting posts the built report and copies the follow-up prompt', () => {
    const source = pane();
    expect(source).toContain("fetch('/workspace/review'");
    expect(source).toContain("method: 'POST'");
    expect(source).toContain("'content-type': 'application/json'");
    expect(source).toContain('filename: reviewFileName(),');
    expect(source).toContain('buildReviewReport(current, { workspace: abs || window.location.origin, generatedAt: Date.now() })');
    expect(source).toContain('navigator.clipboard.writeText(reviewReportPrompt(abs))');
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
    expect(source).toContain("line.kind === 'remove' ? 'old' : 'new'");
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

  test('narrow screens hide the split table and avoid horizontal overflow', () => {
    const css = styles();
    expect(css).toContain('@media (max-width:760px)');
    expect(css).toContain('.review-split-table{display:none}');
    expect(css).toContain('table-layout:fixed');
    expect(css).toContain('overflow-wrap:anywhere');
    expect(css).toContain('.review-gutter{padding:0 3px;font-size:11px}');
  });
});
