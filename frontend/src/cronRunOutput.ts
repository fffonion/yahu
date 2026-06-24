type CronRunOutput = { timestamp?: string; content?: string };

type WaitOptions = {
  attempts?: number;
  delayMs?: number;
};

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function isNewOutput<T extends CronRunOutput>(output: T | null, previousTimestamp = '') {
  if (!output?.content) return false;
  if (!previousTimestamp) return true;
  return Boolean(output.timestamp && output.timestamp !== previousTimestamp);
}

export async function waitForCronRunOutput<T extends CronRunOutput>(
  fetchLatest: () => Promise<T | null>,
  previousTimestamp = '',
  options: WaitOptions = {},
): Promise<T | null> {
  const attempts = Math.max(1, options.attempts ?? 12);
  const delayMs = Math.max(0, options.delayMs ?? 1000);
  let latest: T | null = null;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    latest = await fetchLatest();
    if (isNewOutput(latest, previousTimestamp)) return latest;
    if (attempt < attempts - 1 && delayMs > 0) await delay(delayMs);
  }

  return latest;
}
