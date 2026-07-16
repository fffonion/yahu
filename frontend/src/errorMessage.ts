function safeString(value: unknown): string {
  try {
    return String(value).trim();
  } catch {
    return '';
  }
}

export function errorMessage(error: unknown, fallback?: string): string {
  if (typeof error === 'string') return error || fallback || '';

  try {
    if (error instanceof Error && error.message) return error.message;
    if (error && typeof error === 'object' && 'message' in error) {
      const message = safeString(error.message);
      if (message) return message;
    }
  } catch {
    // Hostile Proxy traps and getters must not escape an error handler.
  }

  if (fallback === undefined && error !== null && error !== undefined) {
    const message = safeString(error);
    if (message) return message;
  }
  return fallback || '';
}

export function isAbortError(error: unknown): boolean {
  try {
    return !!error && typeof error === 'object' && 'name' in error && error.name === 'AbortError';
  } catch {
    return false;
  }
}
