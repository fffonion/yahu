import { describe, expect, test } from 'bun:test';
import { buildChatInputWithAttachments } from './attachmentPayload';

describe('buildChatInputWithAttachments', () => {
  test('includes saved paths for binary uploads instead of saying files were not sent', () => {
    const input = buildChatInputWithAttachments('inspect it', [{
      name: 'report.pdf',
      kind: 'binary',
      mime: 'application/pdf',
      size: 2048,
      uploadedPath: '/home/wow/.hermes/cache/yahu_uploads/report.pdf',
    }]);

    expect(input).toContain('Attached file: report.pdf');
    expect(input).toContain('Saved path: /home/wow/.hermes/cache/yahu_uploads/report.pdf');
    expect(input).not.toContain('not sent');
  });

  test('keeps inline image data while also naming the saved upload path', () => {
    const input = buildChatInputWithAttachments('what is this', [{
      name: 'cat.png',
      kind: 'image',
      mime: 'image/png',
      size: 68,
      dataUrl: 'data:image/png;base64,abc',
      uploadedPath: '/home/wow/.hermes/cache/yahu_uploads/cat.png',
    }]) as any[];

    expect(Array.isArray(input)).toBe(true);
    expect(input[0].text).toContain('Saved path: /home/wow/.hermes/cache/yahu_uploads/cat.png');
    expect(input[1]).toEqual({ type: 'image_url', image_url: { url: 'data:image/png;base64,abc', detail: 'high' } });
  });
});
