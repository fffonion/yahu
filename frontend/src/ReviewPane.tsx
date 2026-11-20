import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import { ChevronDown, Columns2, Copy, GitCompare, Plus, Rows2, X } from 'lucide-react';
import { t, tf } from './i18n';
import { buildSplitRows, newLineAnchors, numberedFileLines, parseUnifiedDiff, type ReviewLine } from './reviewDiff';
import {
  addComment,
  addFile,
  clearReview,
  commentsFor,
  deleteComment,
  loadReview,
  updateComment,
  type ReviewAnchor,
  type ReviewComment,
  type ReviewState,
} from './reviewComments';
import { buildReviewReport, reviewFileName, reviewReportPrompt } from './reviewReport';

export type ReviewPaneProps = {
  path: string;
  onExit: () => void;
  onSubmitted?: (abs: string) => void;
  onFilesChanged?: (files: string[]) => void;
};

type ReviewMode = 'unified' | 'split';

type DiffPayload = {
  path?: string;
  abs?: string;
  repo_path?: string | null;
  diff?: string;
  is_git?: boolean;
  has_head?: boolean;
  truncated?: boolean;
  untracked?: boolean;
  base?: string | null;
};

type CommitEntry = { sha: string; short: string; date: string; subject: string };
type CommitsPayload = { is_git?: boolean; has_head?: boolean; base?: string | null; ahead?: number | null; commits?: CommitEntry[]; has_more?: boolean };
type ReviewRange = 'auto' | 'branch' | `commits:${number}`;

type ReviewUploadPayload = {
  rel?: string;
  abs?: string;
};

type PaneView =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; path: string; repoPath: string | null; lines: ReviewLine[]; hasDiff: boolean; truncated: boolean; untracked: boolean; base: string | null };

type SubmitState =
  | { status: 'idle' }
  | { status: 'submitting' }
  | { status: 'done'; abs: string }
  | { status: 'error'; message: string };

type Draft = { anchor: ReviewAnchor; body: string; commentId?: string; rowKey?: string };

function anchorKey(anchor: ReviewAnchor): string {
  return `${anchor.side}:${anchor.line}`;
}

function lineSign(line: ReviewLine): string {
  return line.kind === 'add' ? '+' : line.kind === 'remove' ? '-' : ' ';
}

function cellTone(line: ReviewLine | undefined): string {
  if (!line) return '';
  if (line.kind === 'add') return 'review-cell-add';
  if (line.kind === 'remove') return 'review-cell-del';
  return '';
}

function statusText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function CommentEditor({ initialBody, onCommit, onCancel }: { initialBody: string; onCommit: (body: string) => void; onCancel: () => void }) {
  const [body, setBody] = useState(initialBody);
  const areaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    const area = areaRef.current;
    if (!area) return;
    area.focus();
    area.setSelectionRange(initialBody.length, initialBody.length);
  }, [initialBody]);

  const commit = () => {
    const trimmed = body.trim();
    if (!trimmed) return;
    onCommit(trimmed);
  };

  return (
    <div className="review-comment-editor">
      <textarea
        ref={areaRef}
        className="review-comment-input"
        value={body}
        placeholder={t('review.commentPlaceholder')}
        onChange={(event) => setBody(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            onCancel();
            return;
          }
          if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            commit();
          }
        }}
      />
      <div className="review-comment-editor-actions">
        <button type="button" className="review-comment-save" onClick={commit}>{t('review.save')}</button>
        <button type="button" className="review-comment-cancel" onClick={onCancel}>{t('review.cancel')}</button>
      </div>
    </div>
  );
}

export function ReviewPane({ path, onExit, onSubmitted, onFilesChanged }: ReviewPaneProps) {
  const [view, setView] = useState<PaneView>({ status: 'loading' });
  const [mode, setMode] = useState<ReviewMode>('unified');
  const [review, setReview] = useState<ReviewState | null>(() => loadReview(window.sessionStorage));
  const [abs, setAbs] = useState('');
  const [draft, setDraft] = useState<Draft | null>(null);
  const [submitState, setSubmitState] = useState<SubmitState>({ status: 'idle' });
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [rangeKey, setRangeKey] = useState<ReviewRange>('auto');
  const [rangeOpen, setRangeOpen] = useState(false);
  const [rangeInfo, setRangeInfo] = useState<CommitsPayload | null>(null);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const rangeRef = useRef<HTMLDivElement | null>(null);
  const rangePathRef = useRef(path);
  const olderRequestRef = useRef(false);
  rangePathRef.current = path;
  const requestRef = useRef(0);

  useEffect(() => {
    setRangeInfo(null);
    setLoadingOlder(false);
    olderRequestRef.current = false;
    if (!path) return;
    let cancelled = false;
    void (async () => {
      try {
        const res = await fetch(`/workspace/commits?path=${encodeURIComponent(path)}`);
        if (!res.ok) return;
        const payload = (await res.json()) as CommitsPayload;
        if (!cancelled) setRangeInfo(payload);
      } catch {
        // The commit list only powers the range menu; review still works without it.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [path]);

  useEffect(() => {
    if (!rangeOpen) return;
    const close = (event: PointerEvent) => {
      if (!(event.target instanceof Element) || !rangeRef.current?.contains(event.target)) setRangeOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [rangeOpen]);

  useEffect(() => {
    const token = requestRef.current + 1;
    requestRef.current = token;
    setDraft(null);
    setSubmitState({ status: 'idle' });
    setCopyState('idle');

    if (!path) {
      setView({ status: 'idle' });
      return;
    }

    setView({ status: 'loading' });

    const load = async () => {
      try {
        const baseParam = rangeKey === 'auto' ? '' : `&base=${encodeURIComponent(rangeKey)}`;
        const res = await fetch(`/workspace/diff?path=${encodeURIComponent(path)}${baseParam}`);
        if (token !== requestRef.current) return;
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        const payload = (await res.json()) as DiffPayload;
        if (token !== requestRef.current) return;
        setAbs(typeof payload.abs === 'string' ? payload.abs : '');
        const stored = addFile(window.sessionStorage, path);
        setReview(stored);
        onFilesChanged?.(stored.files);
        const repoPath = payload.repo_path ?? null;
        const diffText = typeof payload.diff === 'string' ? payload.diff : '';
        if (diffText.trim().length > 0) {
          setView({ status: 'ready', path, repoPath, lines: parseUnifiedDiff(diffText, path), hasDiff: true, truncated: Boolean(payload.truncated), untracked: false, base: typeof payload.base === 'string' ? payload.base : null });
          return;
        }
        const fileRes = await fetch(`/workspace/file?path=${encodeURIComponent(path)}`);
        if (token !== requestRef.current) return;
        if (!fileRes.ok) throw new Error(`${fileRes.status} ${fileRes.statusText}`);
        const content = await fileRes.text();
        if (token !== requestRef.current) return;
        setView({ status: 'ready', path, repoPath, lines: numberedFileLines(content, path), hasDiff: false, truncated: false, untracked: Boolean(payload.untracked), base: typeof payload.base === 'string' ? payload.base : null });
      } catch (error) {
        if (token !== requestRef.current) return;
        setView({ status: 'error', message: statusText(error) });
      }
    };

    void load();
  }, [path, rangeKey]);

  const lines = useMemo<ReviewLine[]>(() => (view.status === 'ready' ? view.lines : []), [view]);
  const newAnchors = useMemo(() => newLineAnchors(lines), [lines]);
  const splitRows = useMemo(() => buildSplitRows(lines), [lines]);

  const rangeCommits = rangeInfo?.commits ?? [];
  const rangeLabel = rangeKey === 'branch'
    ? tf('review.rangeBranch', rangeInfo?.base || 'main')
    : rangeKey === 'auto'
      ? t('review.rangeHead')
      : `HEAD~${rangeKey.split(':')[1]}`;
  const pickRange = (next: ReviewRange) => {
    setRangeOpen(false);
    setRangeKey(next);
  };

  const loadOlderCommits = async () => {
    const currentCount = rangeInfo?.commits?.length ?? 0;
    if (!path || !rangeInfo?.has_more || !currentCount || olderRequestRef.current) return;
    olderRequestRef.current = true;
    setLoadingOlder(true);
    const requestedPath = path;
    try {
      const res = await fetch(`/workspace/commits?path=${encodeURIComponent(path)}&skip=${rangeCommits.length}`);
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const payload = (await res.json()) as CommitsPayload;
      if (rangePathRef.current !== requestedPath) return;
      setRangeInfo((current) => current && current.commits?.length === currentCount
        ? { ...current, commits: [...current.commits, ...(payload.commits ?? [])], has_more: Boolean(payload.has_more) }
        : current);
    } catch {
      // Keep the current options available; the user can retry this page.
    } finally {
      if (rangePathRef.current === requestedPath) {
        olderRequestRef.current = false;
        setLoadingOlder(false);
      }
    }
  };

  const commentCount = review ? review.comments.length : 0;
  const fileComments = useMemo(() => (review ? commentsFor(review, path) : []), [review, path]);
  const grouped = useMemo(() => {
    const map = new Map<string, ReviewComment[]>();
    for (const comment of fileComments) {
      const key = anchorKey(comment.anchor);
      const list = map.get(key) ?? [];
      list.push(comment);
      map.set(key, list);
    }
    return map;
  }, [fileComments]);

  const openDraft = (anchor: ReviewAnchor, comment?: ReviewComment, rowKey?: string) => {
    setDraft({ anchor, body: comment ? comment.body : '', commentId: comment?.id, rowKey });
  };

  const commitDraft = (body: string) => {
    if (!draft) return;
    const next = draft.commentId
      ? updateComment(window.sessionStorage, draft.commentId, body)
      : addComment(window.sessionStorage, path, draft.anchor, body);
    setReview(next);
    setDraft(null);
  };

  const removeComment = (id: string) => {
    setReview(deleteComment(window.sessionStorage, id));
    setDraft(null);
  };

  const submitReview = async () => {
    const current = review;
    if (!current || current.comments.length === 0) return;
    setSubmitState({ status: 'submitting' });
    setCopyState('idle');
    try {
      const filesToRead = [...new Set(current.comments.map((comment) => comment.path))];
      const entries = await Promise.all(filesToRead.map(async (file) => {
        const response = await fetch(`/workspace/file?path=${encodeURIComponent(file)}`);
        if (!response.ok) throw new Error(`${file}: ${response.status} ${response.statusText}`);
        return [file, await response.text()] as const;
      }));
      const contents = new Map(entries);
      const res = await fetch('/workspace/review', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          filename: reviewFileName(),
          path,
          content: buildReviewReport(current, { workspace: abs || window.location.origin, generatedAt: Date.now(), contents }),
        }),
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      let payload: ReviewUploadPayload = {};
      try {
        payload = (await res.json()) as ReviewUploadPayload;
      } catch {
        payload = {};
      }
      const reportPath = typeof payload.abs === 'string' && payload.abs.length > 0 ? payload.abs : window.location.origin;
      clearReview(window.sessionStorage);
      setReview(null);
      setDraft(null);
      setSubmitState({ status: 'done', abs: reportPath });
      onSubmitted?.(reportPath);
    } catch (error) {
      setSubmitState({ status: 'error', message: statusText(error) });
    }
  };

  const copyPrompt = async (abs: string) => {
    const text = reviewReportPrompt(abs);
    let done = false;
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        done = true;
      }
    } catch {
      done = false;
    }
    if (!done) {
      // The async clipboard API rejects without document focus or permission;
      // fall back to the legacy copy path so the button still does something.
      try {
        const area = document.createElement('textarea');
        area.value = text;
        area.setAttribute('readonly', '');
        area.style.position = 'fixed';
        area.style.top = '-1000px';
        area.style.opacity = '0';
        document.body.appendChild(area);
        area.select();
        done = document.execCommand('copy');
        area.remove();
      } catch {
        done = false;
      }
    }
    setCopyState(done ? 'copied' : 'failed');
  };

  const gutterCell = (line: ReviewLine | undefined, side: 'old' | 'new', canComment: boolean, key: string, newNumber?: number, rowKey?: string) => {
    if (!line) return <td className="review-gutter" key={key} />;
    const number = side === 'old' ? line.oldLine : newNumber ?? line.newLine;
    if (number === undefined) return <td className="review-gutter" key={key} />;
    const anchor: ReviewAnchor = { side: 'new', line: number };
    return (
      <td className="review-gutter" key={key}>
        <span className="review-gutter-number">{number}</span>
        {canComment ? (
          <button
            type="button"
            className="review-add-comment"
            aria-label={t('review.addComment')}
            title={t('review.addComment')}
            onClick={() => openDraft(anchor, undefined, rowKey)}
          >
            <Plus aria-hidden="true" />
          </button>
        ) : null}
      </td>
    );
  };

  const markerCell = (line: ReviewLine | undefined, key: string) => (
    <td className={`review-marker ${line ? `review-marker-${line.kind}` : ''}`.trim()} key={key}>
      {line ? lineSign(line) : ' '}
    </td>
  );

  const codeCell = (line: ReviewLine | undefined, side: 'left' | 'right', key: string) => (
    <td className={`review-code-cell ${side} ${cellTone(line)}`.trim()} key={key}>
      {line ? <span className="review-code" dangerouslySetInnerHTML={{ __html: line.html }} /> : null}
    </td>
  );

  const threadRow = (anchors: Array<ReviewAnchor | null>, key: string, columns: number, displayed: Set<string>) => {
    const keys = anchors.filter((anchor): anchor is ReviewAnchor => anchor !== null).map(anchorKey);
    const firstKeys = keys.filter((itemKey) => {
      if (displayed.has(itemKey)) return false;
      displayed.add(itemKey);
      return true;
    });
    const items = firstKeys
      .map((itemKey) => grouped.get(itemKey) ?? [])
      .reduce<ReviewComment[]>((all, list) => all.concat(list), [])
      .sort((left, right) => left.createdAt - right.createdAt);
    const currentDraft = draft && (draft.rowKey ? draft.rowKey === key : firstKeys.includes(anchorKey(draft.anchor))) ? draft : null;
    if (items.length === 0 && !currentDraft) return null;

    return (
      <tr className="review-thread-row" key={`thread-${key}`}>
        <td className="review-thread-cell" colSpan={columns}>
          {items.length > 0 ? (
            <div className="review-thread">
              {items.map((comment) => (
                <div className="review-comment-card" key={comment.id}>
                  <div className="review-comment-head">
                    <span className="review-comment-anchor">{`${comment.anchor.side === 'new' ? 'new' : 'old'} ${comment.anchor.line}`}</span>
                    <span className="review-comment-actions">
                      <button type="button" className="review-comment-edit" onClick={() => openDraft(comment.anchor, comment, key)}>{t('review.edit')}</button>
                      <button type="button" className="review-comment-delete" onClick={() => removeComment(comment.id)}>{t('review.delete')}</button>
                    </span>
                  </div>
                  <p className="review-comment-body">{comment.body}</p>
                </div>
              ))}
            </div>
          ) : null}
          {currentDraft ? (
            <CommentEditor
              key={anchorKey(currentDraft.anchor)}
              initialBody={currentDraft.body}
              onCommit={commitDraft}
              onCancel={() => setDraft(null)}
            />
          ) : null}
        </td>
      </tr>
    );
  };

  const rawRow = (line: ReviewLine, key: string, columns: number) => (
    <tr className={`review-diff-row review-raw-row review-row-${line.kind}`} key={key}>
      <td className="review-full-row" colSpan={columns}>
        <span dangerouslySetInnerHTML={{ __html: line.html }} />
      </td>
    </tr>
  );

  const unifiedDisplayed = new Set<string>();
  const unifiedTable = (
    <table className="review-diff-table review-unified-table">
      <tbody>
        {lines.map((line, index) => {
          if (line.kind === 'file' || line.kind === 'hunk' || line.kind === 'meta') {
            return rawRow(line, `raw-${index}`, 3);
          }
          const newNumber = newAnchors.get(line);
          const anchor: ReviewAnchor | null = newNumber === undefined ? null : { side: 'new', line: newNumber };
          const legacyAnchor: ReviewAnchor | null = line.kind === 'remove' && line.oldLine !== undefined ? { side: 'old', line: line.oldLine } : null;
          const rowClass = line.kind === 'add' ? 'review-row-add' : line.kind === 'remove' ? 'review-row-del' : '';
          return (
            <Fragment key={`line-${index}`}>
              <tr
                className={`review-diff-row review-source-row ${rowClass}`.trim()}
                data-line-old={line.oldLine}
                data-line-new={line.newLine}
              >
                {gutterCell(line, 'old', false, `old-${index}`)}
                {gutterCell(line, 'new', newNumber !== undefined, `new-${index}`, newNumber, `line-${index}`)}
                <td className={`review-code-cell unified ${cellTone(line)}`.trim()} key={`code-${index}`}>
                  <span className="review-code-sign" aria-hidden="true">{lineSign(line)}</span>
                  <span className="review-code" dangerouslySetInnerHTML={{ __html: line.html }} />
                </td>
              </tr>
              {threadRow([anchor, legacyAnchor], `line-${index}`, 3, unifiedDisplayed)}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );

  const splitDisplayed = new Set<string>();
  const splitTable = (
    <table className="review-diff-table review-split-table">
      <tbody>
        {splitRows.map((row) => {
          const left = row.left;
          const right = row.right;
          if (left && (left.kind === 'file' || left.kind === 'hunk' || left.kind === 'meta')) {
            return rawRow(left, `raw-${row.index}`, 6);
          }
          const newNumber = newAnchors.get(right ?? left!);
          const rightAnchor: ReviewAnchor | null = newNumber === undefined ? null : { side: 'new', line: newNumber };
          const legacyAnchor: ReviewAnchor | null = left?.kind === 'remove' && left.oldLine !== undefined ? { side: 'old', line: left.oldLine } : null;
          return (
            <Fragment key={`row-${row.index}`}>
              <tr
                className="review-diff-row review-source-row review-split-row"
                data-line-old={left?.oldLine}
                data-line-new={right?.newLine}
              >
                {gutterCell(left, 'old', false, `left-gutter-${row.index}`)}
                {markerCell(left, `left-marker-${row.index}`)}
                {codeCell(left, 'left', `left-code-${row.index}`)}
                {gutterCell(right ?? left, 'new', newNumber !== undefined, `right-gutter-${row.index}`, newNumber, `row-${row.index}`)}
                {markerCell(right, `right-marker-${row.index}`)}
                {codeCell(right, 'right', `right-code-${row.index}`)}
              </tr>
              {threadRow([rightAnchor, legacyAnchor], `row-${row.index}`, 6, splitDisplayed)}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );

  const displayPath = (view.status === 'ready' && view.path === path ? view.repoPath : null) ?? path.replace(/^@\d+\//, '');
  const notices: string[] = [];
  if (view.status === 'ready') {
    if (view.untracked) notices.push(t('review.untracked'));
    if (!view.hasDiff && view.base) notices.push(tf('review.noChangesInRange', view.base));
    if (!view.hasDiff && !view.untracked && !view.base) notices.push(t('review.noDiff'));
    if (view.truncated) notices.push(t('review.truncated'));
  }

  return (
    <div className="review-pane" data-review-path={path}>
      <div className="review-head">
        <div className="review-toolbar">
          <span className="review-repo-path" title={displayPath}>{displayPath}</span>
          <div className="review-range" ref={rangeRef}>
            <button
              type="button"
              className="review-range-trigger"
              aria-label={t('review.range')}
              aria-haspopup="listbox"
              aria-expanded={rangeOpen}
              title={rangeLabel}
              onClick={() => setRangeOpen((open) => !open)}
            >
              <GitCompare aria-hidden="true" />
              <span>{rangeLabel}</span>
              <ChevronDown aria-hidden="true" />
            </button>
            {rangeOpen && (
              <div className="review-range-menu" role="listbox" aria-label={t('review.range')}>
                {rangeCommits.length > 0 ? (
                  <button type="button" className={`review-range-item${rangeKey === 'commits:1' ? ' active' : ''}`} role="option" aria-selected={rangeKey === 'commits:1'} onClick={() => pickRange('commits:1')}>
                    <span>{t('review.rangeSingle')}</span>
                    <small>{rangeCommits[0].subject} · {rangeCommits[0].short} · {rangeCommits[0].date}</small>
                  </button>
                ) : null}
                {rangeInfo?.base ? (
                  <button type="button" className={`review-range-item${rangeKey === 'branch' ? ' active' : ''}`} role="option" aria-selected={rangeKey === 'branch'} onClick={() => pickRange('branch')}>
                    <span>{tf('review.rangeBranch', rangeInfo.base)}</span>
                    <small>{typeof rangeInfo.ahead === 'number' ? tf('review.rangeAhead', rangeInfo.ahead) : ''}</small>
                  </button>
                ) : null}
                {rangeCommits.length > 1 ? <div className="review-range-sep" role="presentation" /> : null}
                {rangeCommits.slice(1).map((commit, index) => {
                  const nextKey = `commits:${index + 2}` as ReviewRange;
                  return (
                    <button type="button" key={commit.sha} className={`review-range-item${rangeKey === nextKey ? ' active' : ''}`} role="option" aria-selected={rangeKey === nextKey} onClick={() => pickRange(nextKey)}>
                      <span>{tf('review.rangeCount', index + 2)} · {commit.subject}</span>
                      <small>{commit.short} · {commit.date}</small>
                    </button>
                  );
                })}
                {rangeInfo?.has_more ? (
                  <button type="button" className="review-range-load-more" disabled={loadingOlder} onClick={() => void loadOlderCommits()}>
                    {loadingOlder ? t('review.loadingOlder') : t('review.loadOlder')}
                  </button>
                ) : null}
              </div>
            )}
          </div>
          <div className="review-toolbar-actions">
            <div className="review-view-toggle" role="group" aria-label={t('workspace.review')}>
              <button
                type="button"
                className={`review-view-btn${mode === 'unified' ? ' active' : ''}`}
                aria-label={t('review.unified')}
                aria-pressed={mode === 'unified'}
                title={t('review.unified')}
                onClick={() => setMode('unified')}
              >
                <Rows2 aria-hidden="true" />
              </button>
              <button
                type="button"
                className={`review-view-btn${mode === 'split' ? ' active' : ''}`}
                aria-label={t('review.split')}
                aria-pressed={mode === 'split'}
                title={t('review.split')}
                onClick={() => setMode('split')}
              >
                <Columns2 aria-hidden="true" />
              </button>
            </div>
            <button
              type="button"
              className="review-submit"
              disabled={commentCount === 0 || submitState.status === 'submitting'}
              onClick={() => void submitReview()}
            >
              {t('review.submit')}
              <span className="review-submit-count" title={t('review.pending')}>{commentCount}</span>
            </button>
            <button
              type="button"
              className="review-exit-button"
              aria-label={t('review.exit')}
              title={t('review.exit')}
              onClick={onExit}
            >
              <X aria-hidden="true" />
            </button>
          </div>
        </div>
        {notices.length > 0 ? (
          <div className="review-notice">
            {notices.map((notice) => <span key={notice}>{notice}</span>)}
          </div>
        ) : null}
        {submitState.status === 'done' ? (
          <div className="review-submit-result">
            <span className="review-submit-result-label">{t('review.submitted')}</span>
            <code className="review-submit-result-path">{submitState.abs}</code>
            <button
              type="button"
              className="review-copy-prompt"
              aria-label={t('review.copyPrompt')}
              onClick={() => void copyPrompt(submitState.abs)}
            >
              <Copy aria-hidden="true" />
              <span>{copyState === 'copied' ? t('review.copied') : copyState === 'failed' ? t('review.copyFailed') : t('review.copyPrompt')}</span>
            </button>
            {copyState === 'failed' ? (
              <textarea
                className="review-prompt-fallback"
                readOnly
                rows={3}
                value={reviewReportPrompt(submitState.abs)}
                onClick={(event) => event.currentTarget.select()}
                onFocus={(event) => event.currentTarget.select()}
              />
            ) : null}
          </div>
        ) : null}
        {submitState.status === 'error' ? (
          <div className="review-submit-result review-submit-failed">
            <span>{t('review.submitFailed')}</span>
            <span className="review-submit-error-detail">{submitState.message}</span>
          </div>
        ) : null}
      </div>
      <div className="review-diff-body">
        {view.status === 'loading' ? <div className="review-loading">{t('review.loading')}</div> : null}
        {view.status === 'error' ? <div className="review-error">{tf('workspace.previewFailed', view.message)}</div> : null}
        {view.status === 'ready'
          ? (mode === 'split' ? splitTable : unifiedTable)
          : null}
      </div>
    </div>
  );
}

export default ReviewPane;
