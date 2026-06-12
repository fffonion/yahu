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
