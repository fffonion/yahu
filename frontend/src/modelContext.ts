type ProviderModelOption = { id: string; provider?: string; contextLength?: number };

export function selectModelOption<T extends ProviderModelOption>(options: T[], modelId: string, provider = ''): T | undefined {
  const id = modelId.trim();
  const providerId = provider.trim();
  const matches = options.filter((item) => item.id === id);
  if (providerId) return matches.find((item) => String(item.provider || '').trim() === providerId);
  const withContext = matches.filter((item) => Number.isFinite(Number(item.contextLength)) && Number(item.contextLength) > 0);
  return withContext.reduce<T | undefined>((smallest, item) => !smallest || Number(item.contextLength) < Number(smallest.contextLength) ? item : smallest, undefined)
    || matches[0];
}

export function resolvePreferredModelProvider(options: ProviderModelOption[], modelId: string, preferredProvider = ''): string {
  const preferred = preferredProvider.trim();
  const exact = selectModelOption(options, modelId, preferred);
  if (exact) return String(exact.provider || '').trim();
  if (preferred) return preferred;
  return String(selectModelOption(options, modelId)?.provider || '').trim();
}

type ProviderMessage = { model?: string; provider?: string };

export function latestMessageProviderForModel(messages: ProviderMessage[], modelId: string): string {
  const id = modelId.trim();
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    const provider = String(message?.provider || '').trim();
    if (provider && String(message?.model || '').trim() === id) return provider;
  }
  return '';
}

export function fallbackContextWindowForModel(modelId: string, provider = ''): number {
  const id = modelId.toLowerCase();
  const providerId = provider.trim().toLowerCase();
  if (!id) return 128_000;
  if (id.includes('gpt-5.6') && (!providerId || providerId === 'openai-codex')) return 272_000;
  if (id.includes('gpt-5.5') || id.includes('gpt-5.4')) return 1_047_576;
  if (id.includes('claude')) return 200_000;
  if (id.includes('grok-4')) return 256_000;
  if (id.includes('minimax-m3') || id.includes('minimax-m2')) return 1_000_000;
  if (id.includes('gemini')) return 1_000_000;
  return 128_000;
}
