export function errorMessage(error: unknown, fallback = ''): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string' && error) return error;
  if (error && typeof error === 'object' && 'message' in error) {
    const message = String(error.message || '').trim();
    if (message) return message;
  }
  return fallback;
}

export function isAbortError(error: unknown): boolean {
  return !!error && typeof error === 'object' && 'name' in error && error.name === 'AbortError';
}
