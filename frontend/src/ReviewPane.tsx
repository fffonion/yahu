import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import { Copy, Plus, X } from 'lucide-react';
import { t, tf } from './i18n';
import { buildSplitRows, numberedFileLines, parseUnifiedDiff, type ReviewLine } from './reviewDiff';
import {
  addComment,
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
  files: string[];
  onSelectFile: (path: string) => void;
  onRemoveFile: (path: string) => void;
  onExit: () => void;
};

type ReviewMode = 'unified' | 'split';

type DiffPayload = {
  path?: string;
  abs?: string;
  diff?: string;
  is_git?: boolean;
  has_head?: boolean;
  truncated?: boolean;
};

type ReviewUploadPayload = {
  rel?: string;
  abs?: string;
};

type PaneView =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; lines: ReviewLine[]; hasDiff: boolean; truncated: boolean };

type SubmitState =
  | { status: 'idle' }
  | { status: 'submitting' }
  | { status: 'done'; abs: string }
  | { status: 'error'; message: string };

type Draft = { anchor: ReviewAnchor; body: string; commentId?: string };

const COMPACT_QUERY = '(max-width: 760px)';

function anchorKey(anchor: ReviewAnchor): string {
  return `${anchor.side}:${anchor.line}`;
}

function lineSign(line: ReviewLine): string {
  return line.kind === 'add' ? '+' : line.kind === 'remove' ? '-' : ' ';
}

function commentSide(line: ReviewLine): 'new' | 'old' {
  return line.kind === 'remove' ? 'old' : 'new';
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

export function ReviewPane({ path, files, onSelectFile, onRemoveFile, onExit }: ReviewPaneProps) {
  const [view, setView] = useState<PaneView>({ status: 'loading' });
  const [mode, setMode] = useState<ReviewMode>('unified');
  const [narrow, setNarrow] = useState(false);
  const [review, setReview] = useState<ReviewState | null>(() => loadReview(window.sessionStorage));
  const [draft, setDraft] = useState<Draft | null>(null);
  const [submitState, setSubmitState] = useState<SubmitState>({ status: 'idle' });
  const [copied, setCopied] = useState(false);
  const requestRef = useRef(0);

  useEffect(() => {
    const token = requestRef.current + 1;
    requestRef.current = token;
    setDraft(null);

    if (!path) {
      setView({ status: 'idle' });
      return;
    }

    setView({ status: 'loading' });

    const load = async () => {
      try {
        const res = await fetch(`/workspace/diff?path=${encodeURIComponent(path)}`);
        if (token !== requestRef.current) return;
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        const payload = (await res.json()) as DiffPayload;
        if (token !== requestRef.current) return;
        const diffText = typeof payload.diff === 'string' ? payload.diff : '';
        if (diffText.trim().length > 0) {
          setView({ status: 'ready', lines: parseUnifiedDiff(diffText, path), hasDiff: true, truncated: Boolean(payload.truncated) });
          return;
        }
        const fileRes = await fetch(`/workspace/file?path=${encodeURIComponent(path)}`);
        if (token !== requestRef.current) return;
        if (!fileRes.ok) throw new Error(`${fileRes.status} ${fileRes.statusText}`);
        const content = await fileRes.text();
        if (token !== requestRef.current) return;
        setView({ status: 'ready', lines: numberedFileLines(content, path), hasDiff: false, truncated: false });
      } catch (error) {
        if (token !== requestRef.current) return;
        setView({ status: 'error', message: statusText(error) });
      }
    };

    void load();
  }, [path]);

  useEffect(() => {
    const query = window.matchMedia(COMPACT_QUERY);
    const sync = () => setNarrow(query.matches);
    sync();
    query.addEventListener('change', sync);
    return () => query.removeEventListener('change', sync);
  }, []);

  const lines = useMemo<ReviewLine[]>(() => (view.status === 'ready' ? view.lines : []), [view]);
  const splitRows = useMemo(() => buildSplitRows(lines), [lines]);
  const effectiveMode: ReviewMode = narrow ? 'unified' : mode;

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

  const openDraft = (anchor: ReviewAnchor, comment?: ReviewComment) => {
    setDraft({ anchor, body: comment ? comment.body : '', commentId: comment?.id });
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
    setCopied(false);
    try {
      const res = await fetch('/workspace/review', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          filename: reviewFileName(),
          content: buildReviewReport(current, { workspace: window.location.origin, generatedAt: Date.now() }),
        }),
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      let payload: ReviewUploadPayload = {};
      try {
        payload = (await res.json()) as ReviewUploadPayload;
      } catch {
        payload = {};
      }
      const abs = typeof payload.abs === 'string' && payload.abs.length > 0 ? payload.abs : window.location.origin;
      clearReview(window.sessionStorage);
      setReview(null);
      setDraft(null);
      setSubmitState({ status: 'done', abs });
    } catch (error) {
      setSubmitState({ status: 'error', message: statusText(error) });
    }
  };

  const copyPrompt = async (abs: string) => {
    try {
      await navigator.clipboard.writeText(reviewReportPrompt(abs));
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  const gutterCell = (line: ReviewLine | undefined, side: 'old' | 'new', canComment: boolean, key: string) => {
    if (!line) return <td className="review-gutter" key={key} />;
    const number = side === 'old' ? line.oldLine : line.newLine;
    if (number === undefined) return <td className="review-gutter" key={key} />;
    const anchor: ReviewAnchor = { side, line: number };
    return (
      <td className="review-gutter" key={key}>
        <span className="review-gutter-number">{number}</span>
        {canComment ? (
          <button
            type="button"
            className="review-add-comment"
            aria-label={t('review.addComment')}
            title={t('review.addComment')}
            onClick={() => openDraft(anchor)}
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

  const threadRow = (anchors: Array<ReviewAnchor | null>, key: string, columns: number) => {
    const keys = anchors.filter((anchor): anchor is ReviewAnchor => anchor !== null).map(anchorKey);
    const items = keys
      .map((itemKey) => grouped.get(itemKey) ?? [])
      .reduce<ReviewComment[]>((all, list) => all.concat(list), [])
      .sort((left, right) => left.createdAt - right.createdAt);
    const currentDraft = draft && keys.includes(anchorKey(draft.anchor)) ? draft : null;
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
                      <button type="button" className="review-comment-edit" onClick={() => openDraft(comment.anchor, comment)}>{t('review.edit')}</button>
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

  const unifiedTable = (
    <table className="review-diff-table review-unified-table">
      <tbody>
        {lines.map((line, index) => {
          if (line.kind === 'file' || line.kind === 'hunk' || line.kind === 'meta') {
            return rawRow(line, `raw-${index}`, 3);
          }
          const side = commentSide(line);
          const anchor: ReviewAnchor | null = side === 'new'
            ? (line.newLine === undefined ? null : { side, line: line.newLine })
            : (line.oldLine === undefined ? null : { side, line: line.oldLine });
          const rowClass = line.kind === 'add' ? 'review-row-add' : line.kind === 'remove' ? 'review-row-del' : '';
          return (
            <Fragment key={`line-${index}`}>
              <tr
                className={`review-diff-row review-source-row ${rowClass}`.trim()}
                data-line-old={line.oldLine}
                data-line-new={line.newLine}
              >
                {gutterCell(line, 'old', line.kind === 'remove', `old-${index}`)}
                {gutterCell(line, 'new', line.kind !== 'remove', `new-${index}`)}
                <td className={`review-code-cell unified ${cellTone(line)}`.trim()} key={`code-${index}`}>
                  <span className="review-code-sign" aria-hidden="true">{lineSign(line)}</span>
                  <span className="review-code" dangerouslySetInnerHTML={{ __html: line.html }} />
                </td>
              </tr>
              {threadRow([anchor], `line-${index}`, 3)}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );

  const splitTable = (
    <table className="review-diff-table review-split-table">
      <tbody>
        {splitRows.map((row) => {
          const left = row.left;
          const right = row.right;
          if (left && (left.kind === 'file' || left.kind === 'hunk' || left.kind === 'meta')) {
            return rawRow(left, `raw-${row.index}`, 6);
          }
          const leftAnchor: ReviewAnchor | null = left && left.oldLine !== undefined ? { side: 'old', line: left.oldLine } : null;
          const rightAnchor: ReviewAnchor | null = right && right.newLine !== undefined ? { side: 'new', line: right.newLine } : null;
          return (
            <Fragment key={`row-${row.index}`}>
              <tr
                className="review-diff-row review-source-row review-split-row"
                data-line-old={left?.oldLine}
                data-line-new={right?.newLine}
              >
                {gutterCell(left, 'old', left !== undefined && left.oldLine !== undefined, `left-gutter-${row.index}`)}
                {markerCell(left, `left-marker-${row.index}`)}
                {codeCell(left, 'left', `left-code-${row.index}`)}
                {gutterCell(right, 'new', right !== undefined && right.newLine !== undefined, `right-gutter-${row.index}`)}
                {markerCell(right, `right-marker-${row.index}`)}
                {codeCell(right, 'right', `right-code-${row.index}`)}
              </tr>
              {threadRow([leftAnchor, rightAnchor], `row-${row.index}`, 6)}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );

  const notices: string[] = [];
  if (view.status === 'ready') {
    if (!view.hasDiff || lines.length === 0) notices.push(t('review.noDiff'));
    if (view.truncated) notices.push(t('review.truncated'));
  }

  return (
    <div className="review-pane" data-review-path={path}>
      <div className="review-head">
        <div className="review-toolbar">
          {files.length > 0 ? (
            <div className="review-file-chips" role="group" aria-label={t('review.files')}>
              {files.map((file) => (
                <span className={`review-file-chip${file === path ? ' active' : ''}`.trim()} key={file}>
                  <button
                    type="button"
                    className="review-file-chip-open"
                    aria-pressed={file === path}
                    onClick={() => onSelectFile(file)}
                  >
                    {file}
                  </button>
                  <button
                    type="button"
                    className="review-file-chip-remove"
                    aria-label={`${t('review.files')}: ${file}`}
                    onClick={() => onRemoveFile(file)}
                  >
                    <X aria-hidden="true" />
                  </button>
                </span>
              ))}
            </div>
          ) : null}
          <div className="review-view-toggle" role="group" aria-label={t('workspace.review')}>
            <button
              type="button"
              className={`review-view-btn${effectiveMode === 'unified' ? ' active' : ''}`.trim()}
              aria-pressed={effectiveMode === 'unified'}
              onClick={() => setMode('unified')}
            >
              {t('review.unified')}
            </button>
            <button
              type="button"
              className={`review-view-btn${effectiveMode === 'split' ? ' active' : ''}`.trim()}
              aria-pressed={effectiveMode === 'split'}
              disabled={narrow}
              title={narrow ? t('review.unified') : t('review.split')}
              onClick={() => setMode('split')}
            >
              {t('review.split')}
            </button>
          </div>
          <div className="review-toolbar-actions">
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
              <span>{copied ? t('review.copied') : t('review.copyPrompt')}</span>
            </button>
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
          ? (effectiveMode === 'split' ? splitTable : unifiedTable)
          : null}
      </div>
    </div>
  );
}

export default ReviewPane;
