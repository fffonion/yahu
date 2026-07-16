export const DEFAULT_SIDEBAR_WIDTH = 360;
export const MIN_SIDEBAR_WIDTH = 260;
export const MAX_SIDEBAR_WIDTH = 560;
export const SIDEBAR_KEYBOARD_STEP = 16;

export function clampSidebarWidth(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_SIDEBAR_WIDTH;
  return Math.round(Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, value)));
}

export function readSidebarWidth(value: string | null): number {
  if (value === null || value.trim() === '') return DEFAULT_SIDEBAR_WIDTH;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? clampSidebarWidth(parsed) : DEFAULT_SIDEBAR_WIDTH;
}

export function sidebarWidthFromPointer(clientX: number, sidebarLeft: number): number {
  return clampSidebarWidth(clientX - sidebarLeft);
}

export function sidebarWidthFromKey(currentWidth: number, key: string): number {
  if (key === 'ArrowLeft') return clampSidebarWidth(currentWidth - SIDEBAR_KEYBOARD_STEP);
  if (key === 'ArrowRight') return clampSidebarWidth(currentWidth + SIDEBAR_KEYBOARD_STEP);
  if (key === 'Home') return MIN_SIDEBAR_WIDTH;
  if (key === 'End') return MAX_SIDEBAR_WIDTH;
  return currentWidth;
}
