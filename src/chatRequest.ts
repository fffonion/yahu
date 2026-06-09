export type ChatRequestBody = {
  input: unknown;
  model: string;
  provider?: string;
  reasoning_effort: string;
};

export function buildChatRequestBody(input: unknown, model: string, reasoningEffort: string, provider = ''): ChatRequestBody {
  const body: ChatRequestBody = {
    input,
    model,
    reasoning_effort: reasoningEffort,
  };
  const providerId = provider.trim();
  if (providerId) body.provider = providerId;
  return body;
}
