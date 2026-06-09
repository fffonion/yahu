import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

describe('chat attachment upload wiring', () => {
  const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

  test('uploads selected files to yahu local cache before streaming to Hermes API Server', () => {
    expect(source).toContain("fetch('/chat/attachments'");
    expect(source).toContain('payloadAttachments = await uploadAttachments(attachments)');
    expect(source).toContain('buildChatInputWithAttachments(input, items)');
  });

  test('does not route yahu attachment cache uploads through the Hermes API proxy', () => {
    expect(source).not.toContain("apiJoin(apiBase, '/chat/attachments')");
    expect(source).not.toContain('Attached binary file not sent through Hermes API Server');
  });
});
