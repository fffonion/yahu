import { describe, expect, test } from 'bun:test';
import { buildHashRoute, parseHashRoute } from './hashRoute';

describe('hash route helpers', () => {
  test('parses and builds chat session routes', () => {
    expect(parseHashRoute('#/chat/session-123')).toEqual({ mode: 'chat', sessionId: 'session-123' });
    expect(buildHashRoute({ mode: 'chat', sessionId: 'session-123' })).toBe('#/chat/session-123');
  });

  test('parses and builds cron item routes', () => {
    expect(parseHashRoute('#/cron/job%2Fabc')).toEqual({ mode: 'cron', jobId: 'job/abc' });
    expect(buildHashRoute({ mode: 'cron', jobId: 'job/abc' })).toBe('#/cron/job%2Fabc');
  });

  test('parses and builds skill routes only when a skill name is present', () => {
    expect(parseHashRoute('#/skills')).toEqual({ mode: 'skills' });
    expect(parseHashRoute('#/skills/anime-torrent-download')).toEqual({ mode: 'skills', skillName: 'anime-torrent-download' });
    expect(buildHashRoute({ mode: 'skills' })).toBe('#/skills');
    expect(buildHashRoute({ mode: 'skills', skillName: 'anime-torrent-download' })).toBe('#/skills/anime-torrent-download');
  });

  test('parses and builds image modal routes', () => {
    expect(parseHashRoute('#/images/openai_image.png')).toEqual({ mode: 'images', imageFilename: 'openai_image.png' });
    expect(buildHashRoute({ mode: 'images', imageFilename: 'openai image.png' })).toBe('#/images/openai%20image.png');
  });

  test('parses and builds workspace file and folder routes with slash paths', () => {
    expect(parseHashRoute('#/workspace/file/src%2FApp.tsx')).toEqual({ mode: 'workspace', workspaceKind: 'file', workspacePath: 'src/App.tsx' });
    expect(parseHashRoute('#/workspace/folder/src%2Fcomponents')).toEqual({ mode: 'workspace', workspaceKind: 'folder', workspacePath: 'src/components' });
    expect(buildHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: 'src/App.tsx' })).toBe('#/workspace/file/src%2FApp.tsx');
  });

  test('parses memory insights artifacts and settings routes', () => {
    expect(parseHashRoute('#/memory')).toEqual({ mode: 'memory' });
    expect(parseHashRoute('#/insights')).toEqual({ mode: 'insights' });
    expect(parseHashRoute('#/artifacts')).toEqual({ mode: 'artifacts' });
    expect(parseHashRoute('#/artifacts/artifact-s1')).toEqual({ mode: 'artifacts', artifactId: 'artifact-s1' });
    expect(parseHashRoute('#/settings')).toEqual({ mode: 'settings' });
    expect(buildHashRoute({ mode: 'memory' })).toBe('#/memory');
    expect(buildHashRoute({ mode: 'insights' })).toBe('#/insights');
    expect(buildHashRoute({ mode: 'artifacts' })).toBe('#/artifacts');
    expect(buildHashRoute({ mode: 'artifacts', artifactId: 'artifact/s1' })).toBe('#/artifacts/artifact%2Fs1');
    expect(buildHashRoute({ mode: 'settings' })).toBe('#/settings');
  });
});
