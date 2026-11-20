import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
const app = () => source('App.tsx');
const transcript = () => source('ChatTranscript.tsx');
const reviewPane = () => source('ReviewPane.tsx');

function slice(text: string, start: string, end: string): string {
  const from = text.indexOf(start);
  if (from < 0) return '';
  const to = text.indexOf(end, from + start.length);
  return text.slice(from, to < 0 ? text.length : to);
}

describe('workspace review wiring', () => {
  test('the subagent overlay dispatches review clicks outside the chat scroller', () => {
    const text = app();
    expect(text).toContain('const onSubagentAreaClick = (event: React.MouseEvent<HTMLElement>) => {');
    expect(text).toContain("const reviewTarget = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-workspace-review]') : null;");
    expect(text).toContain('if (reviewPath) props.onStartWorkspaceReview(reviewPath);');
    expect(text).toContain('<div className="subagent-progress-overlay" onWheel={onStatusOverlayWheel} onClick={onSubagentAreaClick}>');
  });

  test('App imports ReviewPane and tracks a review session', () => {
    const text = app();
    expect(text).toContain("import { ReviewPane } from './ReviewPane';");
    expect(text).toContain('const [reviewSession, setReviewSession] = useState<{ active: boolean; path: string; files: string[] } | null>(null);');
    expect(text).toContain('const startWorkspaceReview = useCallback(async (path: string) =>');
    expect(text).toContain('const selectReviewFile = useCallback(async (path: string) =>');
    expect(text).toContain('const removeReviewFile = useCallback((path: string) =>');
    expect(text).toContain('const exitWorkspaceReview = useCallback(() => { workspaceReviewRequest.current += 1; setReviewSession(null); }, []);');
    expect(text).toContain('const submitWorkspaceReview = useCallback(() => {');
  });

  test('starting a review jumps to the workspace page and loads the file without a second hash write', () => {
    const text = slice(app(), 'const startWorkspaceReview = useCallback', 'const selectReviewFile = useCallback');
    expect(text).toContain("setMode('workspace')");
    expect(text).toContain('setSidebarCollapsed(false)');
    expect(text).toContain("const expanded = path.startsWith('~/') ? roots[0].path + path.slice(1) : path;");
    expect(text).toContain('const relative = workspaceRootPrefix(expanded, roots);');
    expect(text).toContain("writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: targetPath })");
    expect(text).toContain('setWorkspaceRouteTarget({ workspaceKind: \'file\', workspacePath: targetPath })');
    expect(text).toContain("openWorkspaceEntry({ name: basename(targetPath), path: targetPath, kind: 'file' }, { route: false, onLoaded:");
    expect(text).toContain('setReviewSession((old) => ({ active: true, path: targetPath, files:');
  });

  test('a non-text preview never enters review', () => {
    const text = slice(app(), 'const startWorkspaceReview = useCallback', 'const selectReviewFile = useCallback');
    expect(text).toContain('if (!isCurrent() || !loadedKind) return;');
    expect(text).toContain("if (loadedKind !== 'text')");
    expect(text).toContain('setReviewSession(null);');
    expect(text).toContain("t('review.notTextFile')");
  });

  test('removing the current file falls back to the next remaining file', () => {
    const text = slice(app(), 'const removeReviewFile = useCallback', 'const exitWorkspaceReview = useCallback');
    expect(text).toContain('const stored = removeReviewStorageFile(storage, path);');
    expect(text).toContain('const files = stored.files;');
    expect(text).toContain('if (files.length === 0) { setReviewSession(null); return; }');
    expect(text).toContain('const nextPath = !current || current.path === path ? files[0] : current.path;');
  });

  test('the workspace header offers a Review toggle next to Edit', () => {
    const text = app();
    expect(text).toContain('className="icon-btn workspace-review-toggle"');
    expect(text).toContain("aria-label={t('workspace.review')} title={t('workspace.review')} onClick={() => onToggleReview?.(preview.path)}");
    expect(text).toContain("{!readOnly && !editMode && preview.kind === 'text' && !review?.active && <><button className=\"icon-btn workspace-review-toggle\"");
  });

  test('ReviewPane replaces the read only preview and is gated to text files', () => {
    const text = app();
    const preview = slice(text, 'function WorkspaceEditorPreview', 'type WorkspaceBrowserProps');
    expect(preview.includes("if (review?.active && preview.kind === 'text' && !editMode)")).toBe(true);
    const tabs = slice(preview, 'className="preview-head review-file-tabs"', '<ReviewPane');
    expect(tabs.includes('{basename(file)}')).toBe(true);
    expect(tabs.includes('onSelectReviewFile?.(file)')).toBe(true);
    expect(tabs.includes('onRemoveReviewFile?.(file)')).toBe(true);
    expect(text).toContain('onExit={() => onExitReview?.()}');
    expect(text).toContain("onSubmitted={onSubmittedReview}");
    expect(text).toContain('onSubmittedReview={submitWorkspaceReview}');
  });

  test('WorkspaceMain receives the review props and ChatMain receives the starter', () => {
    const text = app();
    expect(text).toContain('review={reviewSession} onToggleReview={startWorkspaceReview}');
    expect(text).toContain('onSelectReviewFile={selectReviewFile} onRemoveReviewFile={removeReviewFile}');
    expect(text).toContain('onExitReview={exitWorkspaceReview}');
    expect(text).toContain('onStartWorkspaceReview={startWorkspaceReview}');
    expect(text).toContain('onStartWorkspaceReview: (path: string) => void;');
  });

  test('chat clicks dispatch the review button before the tool path fallback', () => {
    const text = slice(app(), 'const onChatAreaClick', 'const contextWindowTotal');
    expect(text).toContain("closest<HTMLElement>('[data-workspace-review]')");
    expect(text).toContain("getAttribute('data-workspace-review')");
    expect(text).toContain('props.onStartWorkspaceReview(reviewPath);');
    expect(text.indexOf('[data-workspace-review]')).toBeLessThan(text.indexOf('[data-tool-path]'));
    expect(text).toContain('event.preventDefault();');
    expect(text).toContain('event.stopPropagation();');
  });

  test('tool cards expose a review entry only when they carry a workspace path', () => {
    const text = transcript();
    expect(text).toContain("import { toolWorkspacePath } from './workspaceToolPath';");
    expect(text).toContain('const reviewPath = toolWorkspacePath(summary);');
    expect(text).toContain('data-tool-path={reviewPath ?? undefined}');
    expect(text).toContain('data-workspace-review={reviewPath}');
    expect(text).toContain("title={t('workspace.review')} aria-label={t('workspace.review')}");
    expect(text).toContain('{reviewPath ? <button type="button" className="icon-btn tool-review-btn"');
  });

  test('review state stays in memory and comments stay in sessionStorage', () => {
    const feature = slice(app(), 'const startWorkspaceReview = useCallback', 'const revealWorkspaceToolPath');
    expect(feature).not.toContain('localStorage');
    expect(transcript()).not.toContain('localStorage');
    expect(reviewPane()).not.toContain('localStorage');
    expect(reviewPane()).toContain('loadReview(window.sessionStorage)');
    expect(reviewPane()).toContain('clearReview(window.sessionStorage)');
  });

  test('ReviewPane reports a successful submission through the optional callback', () => {
    const text = reviewPane();
    expect(text).toContain('onSubmitted?: (abs: string) => void;');
    expect(text).toContain("setSubmitState({ status: 'done', abs: reportPath });");
    expect(text).toContain('onSubmitted?.(reportPath);');
  });
});
