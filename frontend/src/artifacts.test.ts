import { describe, expect, test } from 'bun:test';
import { artifactCopyPrompt, buildSessionArtifact, copyTextToClipboard } from './artifacts';

describe('session artifacts', () => {
  test('builds a shareable session artifact with summary, timeline, and copy prompt', () => {
    const artifact = buildSessionArtifact({
      session: { id: 's1', title: 'Debug flaky editor', startedAt: 1710000000000 },
      messages: [
        { id: 'm1', role: 'user', content: 'Cursor is offset in the editor', timestamp: 1710000000000 },
        { id: 'm2', role: 'assistant', content: 'I reproduced it and found the overlay mismatch.', timestamp: 1710000060000 },
        { id: 'm3', role: 'tool', content: 'bun test -> 272 pass', toolName: 'terminal', timestamp: 1710000120000 },
      ],
      now: 1710000200000,
    });

    expect(artifact.id).toBe('artifact-s1');
    expect(artifact.title).toBe('Debug flaky editor');
    expect(artifact.sourceSessionId).toBe('s1');
    expect(artifact.versions).toHaveLength(1);
    expect(artifact.versions[0].summary.totalMessages).toBe(3);
    expect(artifact.versions[0].summary.toolMessages).toBe(1);
    expect(artifact.versions[0].timeline.map((item) => item.title)).toEqual(['Request', 'Work summary', 'Tool evidence']);
    expect(artifact.versions[0].timeline.map((item) => item.excerpt)).not.toEqual([
      'Cursor is offset in the editor',
      'I reproduced it and found the overlay mismatch.',
      'bun test -> 272 pass',
    ]);
    expect(artifactCopyPrompt(artifact)).toContain('Timeline:');
    expect(artifactCopyPrompt(artifact)).toContain('Continue from artifact “Debug flaky editor”');
    expect(artifactCopyPrompt(artifact)).toContain('s1');
  });

  test('republishes the same session as a new version at the same artifact id', () => {
    const first = buildSessionArtifact({
      session: { id: 's1', title: 'Release checklist' },
      messages: [{ id: 'm1', role: 'user', content: 'Draft release checklist' }],
      now: 1710000000000,
    });
    const second = buildSessionArtifact({
      session: { id: 's1', title: 'Release checklist' },
      messages: [
        { id: 'm1', role: 'user', content: 'Draft release checklist' },
        { id: 'm2', role: 'assistant', content: 'Added deployment verification.' },
      ],
      existing: first,
      now: 1710000100000,
    });

    expect(second.id).toBe(first.id);
    expect(second.versions).toHaveLength(2);
    expect(second.versions[0].version).toBe(1);
    expect(second.versions[1].version).toBe(2);
    expect(second.versions[1].summary.assistantMessages).toBe(1);
    expect(artifactCopyPrompt(second, second.versions[0])).toContain('Version: 1');
    expect(artifactCopyPrompt(second, second.versions[1])).toContain('Version: 2');
  });

  test('copy helper falls back to a hidden textarea when clipboard API is unavailable', async () => {
    const calls: string[] = [];
    const textarea = {
      value: '',
      style: {},
      focus: () => calls.push('focus'),
      select: () => calls.push('select'),
      remove: () => calls.push('remove'),
    };
    const ok = await copyTextToClipboard('artifact prompt', {
      navigator: {},
      document: {
        createElement: () => textarea,
        body: { appendChild: (node: any) => calls.push(node === textarea ? 'append' : 'other') },
        execCommand: (command: string) => command === 'copy',
      } as any,
    });

    expect(ok).toBe(true);
    expect(textarea.value).toBe('artifact prompt');
    expect(calls).toEqual(['append', 'focus', 'select', 'remove']);
  });
});
