export type ImageNavigationItem = { filename: string };

export function nextImageAfterRemoval<T extends ImageNavigationItem>(items: T[], removedNames: string[], currentFilename?: string | null): T | null {
  if (!currentFilename) return null;
  const removed = new Set(removedNames.filter(Boolean));
  const currentIndex = items.findIndex((item) => item.filename === currentFilename);
  if (currentIndex < 0) return null;
  if (!removed.has(currentFilename)) return items[currentIndex] || null;
  const remaining = items.filter((item) => !removed.has(item.filename));
  if (!remaining.length) return null;
  const nextAtSameIndex = remaining[currentIndex];
  if (nextAtSameIndex) return nextAtSameIndex;
  return remaining[remaining.length - 1] || null;
}

export function nextImageForPreload<T extends ImageNavigationItem>(items: T[], currentFilename?: string | null): T | null {
  if (!currentFilename) return null;
  const currentIndex = items.findIndex((item) => item.filename === currentFilename);
  if (currentIndex < 0) return null;
  return items[currentIndex + 1] || null;
}
