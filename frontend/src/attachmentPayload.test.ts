import { describe, expect, test } from 'bun:test';
import { buildChatInputWithAttachments } from './attachmentPayload';

describe('buildChatInputWithAttachments', () => {
  test('includes saved paths for binary uploads instead of saying files were not sent', () => {
    const input = buildChatInputWithAttachments('inspect it', [{
      name: 'report.pdf',
      kind: 'binary',
      mime: 'application/pdf',
      size: 2048,
      uploadedPath: '/workspace/example/uploads/report.pdf',
    }]);

    expect(input).toContain('Attached file: report.pdf');
    expect(input).toContain('Saved path: /workspace/example/uploads/report.pdf');
    expect(input).not.toContain('not sent');
  });

  test('keeps inline image data while also naming the saved upload path', () => {
    const input = buildChatInputWithAttachments('what is this', [{
      name: 'cat.png',
      kind: 'image',
      mime: 'image/png',
      size: 68,
      dataUrl: 'data:image/png;base64,abc',
      uploadedPath: '/workspace/example/uploads/cat.png',
    }]) as any[];

    expect(Array.isArray(input)).toBe(true);
    expect(input[0].text).toContain('Saved path: /workspace/example/uploads/cat.png');
    expect(input[1]).toEqual({ type: 'image_url', image_url: { url: 'data:image/png;base64,abc', detail: 'high' } });
  });
});
