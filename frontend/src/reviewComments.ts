export type ReviewAnchor = { side: 'new' | 'old'; line: number };
export type ReviewComment = { id: string; path: string; anchor: ReviewAnchor; body: string; createdAt: number; updatedAt: number };
export type ReviewState = { id: string; createdAt: number; files: string[]; comments: ReviewComment[] };

export const REVIEW_STORAGE_KEY = 'yahu.review.v1';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isReviewAnchor(value: unknown): value is ReviewAnchor {
  return isRecord(value)
    && (value.side === 'new' || value.side === 'old')
    && isFiniteNumber(value.line);
}

function isReviewComment(value: unknown): value is ReviewComment {
  return isRecord(value)
    && typeof value.id === 'string'
    && typeof value.path === 'string'
    && isReviewAnchor(value.anchor)
    && typeof value.body === 'string'
    && isFiniteNumber(value.createdAt)
    && isFiniteNumber(value.updatedAt);
}

function isReviewState(value: unknown): value is ReviewState {
  return isRecord(value)
    && typeof value.id === 'string'
    && isFiniteNumber(value.createdAt)
    && Array.isArray(value.files)
    && value.files.every((path) => typeof path === 'string')
    && Array.isArray(value.comments)
    && value.comments.every(isReviewComment);
}

function randomPart(): string {
  return Math.random().toString(36).slice(2) || '0';
}

function createReview(now: number): ReviewState {
  return { id: `rv_${now}_${randomPart()}`, createdAt: now, files: [], comments: [] };
}

function persist(storage: Storage, state: ReviewState): ReviewState {
  storage.setItem(REVIEW_STORAGE_KEY, JSON.stringify(state));
  return state;
}

function currentOrStart(storage: Storage): ReviewState {
  return loadReview(storage) ?? startReview(storage);
}

export function loadReview(storage: Storage): ReviewState | null {
  try {
    const raw = storage.getItem(REVIEW_STORAGE_KEY);
    if (raw === null) return null;
    const parsed: unknown = JSON.parse(raw);
    return isReviewState(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function startReview(storage: Storage, now = Date.now()): ReviewState {
  return persist(storage, createReview(now));
}

export function addFile(storage: Storage, path: string): ReviewState {
  const state = currentOrStart(storage);
  if (!state.files.includes(path)) state.files.push(path);
  return persist(storage, state);
}

export function removeFile(storage: Storage, path: string): ReviewState {
  const state = currentOrStart(storage);
  // Drop the file from the review list but keep its comments: removing a chip
  // must not silently delete text the user already wrote.
  state.files = state.files.filter((filePath) => filePath !== path);
  return persist(storage, state);
}

export function addComment(storage: Storage, path: string, anchor: ReviewAnchor, body: string): ReviewState {
  const trimmedBody = body.trim();
  if (!trimmedBody) return loadReview(storage) ?? createReview(Date.now());

  const state = currentOrStart(storage);
  const now = Date.now();
  state.comments.push({
    id: `c_${now}_${randomPart()}`,
    path,
    anchor,
    body: trimmedBody,
    createdAt: now,
    updatedAt: now,
  });
  return persist(storage, state);
}

export function updateComment(storage: Storage, id: string, body: string): ReviewState {
  const trimmedBody = body.trim();
  if (!trimmedBody) return loadReview(storage) ?? createReview(Date.now());
  const state = currentOrStart(storage);
  const comment = state.comments.find((item) => item.id === id);
  if (!comment) return state;
  comment.body = trimmedBody;
  comment.updatedAt = Date.now();
  return persist(storage, state);
}

export function deleteComment(storage: Storage, id: string): ReviewState {
  const state = currentOrStart(storage);
  const remaining = state.comments.filter((comment) => comment.id !== id);
  if (remaining.length === state.comments.length) return state;
  state.comments = remaining;
  return persist(storage, state);
}

export function commentsFor(state: ReviewState, path: string): ReviewComment[] {
  return state.comments
    .filter((comment) => comment.path === path)
    .sort((left, right) => left.anchor.line - right.anchor.line || left.createdAt - right.createdAt);
}

export function clearReview(storage: Storage): void {
  storage.removeItem(REVIEW_STORAGE_KEY);
}
