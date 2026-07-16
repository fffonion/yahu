import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { formatFileSize } from './formatFileSize';

const appSource = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const attachmentSource = readFileSync(new URL('./attachmentPayload.ts', import.meta.url), 'utf8');

describe('frontend boundary cleanup', () => {
  test('shares one file-size formatter', () => {
    expect(formatFileSize(undefined)).toBe('');
    expect(formatFileSize(1023)).toBe('1023 B');
    expect(formatFileSize(1024)).toBe('1.0 k');
    expect(formatFileSize(1024 * 1024)).toBe('1.0 M');
    expect(appSource).not.toContain('const fmtSize =');
    expect(attachmentSource).not.toContain('const fmtSize =');
  });

  test('keeps history and image metadata boundaries typed', () => {
    expect(appSource).not.toContain('data: any[]');
    expect(appSource).not.toContain('(items: any[] | null | undefined)');
    expect(appSource).not.toContain('webp?: unknown');
    expect(appSource).not.toContain('(metadata.webp as any).size');
    expect(appSource).not.toContain('(metadata.heic as any).size');
  });

  test('does not compute and discard hash routes', () => {
    expect(appSource).not.toContain("buildHashRoute({ mode: 'cron', jobId: jobId(job) });");
    expect(appSource).not.toContain("if (options?.route !== false) buildHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });");
    expect(appSource).not.toContain("writeHashRoute({ mode: 'images', imageFilename: item.filename }); buildHashRoute({ mode: 'images', imageFilename: item.filename });");
  });
});
