export type ChatScrollPosition = {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
};

export const CHAT_LATEST_THRESHOLD_PX = 120;

export function chatLatestButtonVisible(
  position: ChatScrollPosition | null | undefined,
  hasNewer: boolean,
): boolean {
  if (hasNewer) return true;
  if (!position) return false;
  return position.scrollHeight - position.scrollTop - position.clientHeight > CHAT_LATEST_THRESHOLD_PX;
}
