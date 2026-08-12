export type ModelOption = { id: string; label: string; provider?: string; contextLength?: number };

const PROVIDER_LABELS: Record<string, string> = {
  'openai-codex': 'Codex',
  minimax: 'MiniMax',
  'minimax-cn': 'MiniMax',
  copilot: 'GitHub Copilot',
  openrouter: 'OpenRouter',
  anthropic: 'Anthropic',
  openai: 'OpenAI',
  deepseek: 'DeepSeek',
  xai: 'xAI',
  'google-gemini': 'Google Gemini',
  gemini: 'Google Gemini',
};

export function providerDisplayName(provider: string) {
  const id = provider.trim();
  if (!id) return '';
  return PROVIDER_LABELS[id] || id.replace(/^custom:/, '').split(/[-_]/).filter(Boolean).map((part) => part[0]?.toUpperCase() + part.slice(1)).join(' ');
}

export function currentModelDisplayOption(modelId: string, options: ModelOption[], apiProvider?: string): ModelOption {
  const id = modelId.trim();
  void options;
  void apiProvider;
  return { id, label: id };
}

function isMixtureOfAgentsOption(item: ModelOption) {
  return `${item.provider || ''} ${item.id} ${item.label}`.toLowerCase().replace(/[_-]+/g, ' ').includes('mixture of agents');
}

export function orderedModelOptions(options: ModelOption[], currentModel: string, currentProvider: string) {
  const currentId = currentModel.trim();
  const providerId = currentProvider.trim();
  const normalized = options.map((option) => option.id === currentId && providerId && !String(option.provider || '').trim() ? { ...option, provider: providerId } : option);
  const unique = new Map<string, ModelOption>();
  for (const option of normalized) unique.set(`${String(option.provider || '').trim()}\u0000${option.id}`, option);
  return Array.from(unique.values()).sort((a, b) => {
    const aCurrent = a.id === currentId && (!providerId || !a.provider || a.provider === providerId);
    const bCurrent = b.id === currentId && (!providerId || !b.provider || b.provider === providerId);
    if (aCurrent !== bCurrent) return aCurrent ? -1 : 1;
    const aMixture = !aCurrent && isMixtureOfAgentsOption(a);
    const bMixture = !bCurrent && isMixtureOfAgentsOption(b);
    if (aMixture !== bMixture) return aMixture ? 1 : -1;
    const aSameProvider = !aCurrent && !!providerId && String(a.provider || '').trim() === providerId;
    const bSameProvider = !bCurrent && !!providerId && String(b.provider || '').trim() === providerId;
    if (aSameProvider !== bSameProvider) return aSameProvider ? -1 : 1;
    const aKey = `${String(a.provider || '').trim()} - ${a.id}`;
    const bKey = `${String(b.provider || '').trim()} - ${b.id}`;
    return aKey.localeCompare(bKey, undefined, { numeric: true, sensitivity: 'base' });
  });
}

export function modelTriggerLabel(modelId: string, mobile: boolean) {
  const value = modelId.trim();
  if (!mobile) return value;
  const parts = value.split('/').map((part) => part.trim()).filter(Boolean);
  return parts[parts.length - 1] || value;
}

export function modelMenuLabel(item: ModelOption) {
  const provider = String(item.provider || '').trim();
  return provider ? `${providerDisplayName(provider)} - ${item.id}` : item.label;
}
