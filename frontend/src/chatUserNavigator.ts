export type NavigatorItem = { id: string };

/** Return one current minimap line from the visible user-message range. */
export function currentNavigatorId(items: ReadonlyArray<NavigatorItem>, visibleIds: ReadonlySet<string>): string | undefined {
  let current: string | undefined;
  for (const item of items) {
    if (visibleIds.has(item.id)) current = item.id;
  }
  return current;
}
