import { describe, expect, test } from 'bun:test';
import {
  REVIEW_STORAGE_KEY,
  addComment,
  addFile,
  clearReview,
  commentsFor,
  deleteComment,
  loadReview,
  removeFile,
  startReview,
  updateComment,
} from './reviewComments';

class MemoryStorage implements Storage {
  private readonly values = new Map<string, string>();

  get length() {
    return this.values.size;
  }

  clear() {
    this.values.clear();
  }

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  key(index: number) {
    return [...this.values.keys()][index] ?? null;
  }

  removeItem(key: string) {
    this.values.delete(key);
  }

  setItem(key: string, value: string) {
    this.values.set(key, String(value));
  }
}

describe('review comment storage', () => {
  test('persists a review through file and comment lifecycle operations', () => {
    const storage = new MemoryStorage();
    const createdAt = 1_700_000_000_123;
    const started = startReview(storage, createdAt);

    expect(started).toEqual({ id: expect.stringMatching(/^rv_1700000000123_[a-z0-9]+$/), createdAt, files: [], comments: [] });
    expect(loadReview(storage)).toEqual(started);

    addFile(storage, 'src/z.ts');
    addFile(storage, 'src/a.ts');
    const afterDuplicate = addFile(storage, 'src/z.ts');
    expect(afterDuplicate.files).toEqual(['src/z.ts', 'src/a.ts']);

    const first = addComment(storage, 'src/z.ts', { side: 'new', line: 8 }, '  later line  ');
    const firstComment = first.comments[0];
    expect(firstComment?.body).toBe('later line');
    expect(firstComment?.id).toMatch(/^c_\d+_[a-z0-9]+$/);

    addComment(storage, 'src/z.ts', { side: 'old', line: 3 }, 'earlier line');
    const ordered = commentsFor(loadReview(storage)!, 'src/z.ts');
    expect(ordered.map(({ anchor }) => anchor.line)).toEqual([3, 8]);

    const updated = updateComment(storage, firstComment!.id, 'updated note');
    const updatedComment = updated.comments.find(({ id }) => id === firstComment!.id);
    expect(updatedComment?.body).toBe('updated note');
    expect(updatedComment?.updatedAt).toBeGreaterThanOrEqual(firstComment!.createdAt);

    const deleted = deleteComment(storage, firstComment!.id);
    expect(deleted.comments.some(({ id }) => id === firstComment!.id)).toBe(false);
    expect(deleteComment(storage, 'missing-comment')).toEqual(deleted);

    addComment(storage, 'src/z.ts', { side: 'new', line: 12 }, 'remove with file');
    const removed = removeFile(storage, 'src/z.ts');
    expect(removed.files).toEqual(['src/a.ts']);
    expect(removed.comments.every(({ path }) => path !== 'src/z.ts')).toBe(true);
    expect(loadReview(storage)).toEqual(removed);

    clearReview(storage);
    expect(storage.getItem(REVIEW_STORAGE_KEY)).toBeNull();
    expect(loadReview(storage)).toBeNull();
  });

  test('returns null for malformed JSON and structurally invalid stored state', () => {
    const storage = new MemoryStorage();
    storage.setItem(REVIEW_STORAGE_KEY, '{broken');
    expect(loadReview(storage)).toBeNull();
    storage.setItem(REVIEW_STORAGE_KEY, JSON.stringify({ id: 42, files: [], comments: [] }));
    expect(loadReview(storage)).toBeNull();
  });

  test('rejects blank comment bodies without changing stored state', () => {
    const storage = new MemoryStorage();
    const started = startReview(storage, 123);
    const before = storage.getItem(REVIEW_STORAGE_KEY);

    expect(addComment(storage, 'src/file.ts', { side: 'new', line: 1 }, ' \n\t ')).toEqual(started);
    expect(storage.getItem(REVIEW_STORAGE_KEY)).toBe(before);
  });

  test('updating an unknown comment id leaves the review unchanged', () => {
    const storage = new MemoryStorage();
    startReview(storage, 456);
    addFile(storage, 'src/file.ts');
    const before = loadReview(storage);

    expect(updateComment(storage, 'unknown-id', 'unused')).toEqual(before!);
  });
});
