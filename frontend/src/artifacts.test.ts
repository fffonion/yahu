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

  test('extracts high-value tool outputs into evidence instead of restyling raw transcript', () => {
    const artifact = buildSessionArtifact({
      session: { id: 'tools1', title: 'Artifacts implementation' },
      messages: [
        { id: 'u1', role: 'user', content: 'Make artifacts useful.' },
        { id: 'rf', role: 'tool', toolName: 'read_file', content: JSON.stringify({ tool_name: 'read_file', path: '/tmp/App.tsx', result: 'read 120 lines' }) },
        { id: 'test', role: 'tool', toolName: 'functions.terminal', content: JSON.stringify({ tool_name: 'terminal', arguments: { command: 'bun test' }, output: '278 pass\n0 fail\nRan 278 tests across 62 files.', exit_code: 0 }) },
        { id: 'browser', role: 'tool', toolName: 'browser_snapshot', content: '<untrusted_tool_result source="browser_snapshot">heading "Artifacts"\nbutton "Copy as prompt"\nparagraph: Tool evidence</untrusted_tool_result>' },
        { id: 'a1', role: 'assistant', content: 'Implemented a real artifact canvas with evidence.' },
      ],
      now: 1710000300000,
    });
    const version = artifact.versions[0];

    expect(version.evidence.map((item) => item.toolName).slice(0, 2)).toEqual(['terminal', 'browser_snapshot']);
    expect(version.evidence[0].category).toBe('verification');
    expect(version.evidence[0].findings).toContain('278 tests passed');
    expect(version.evidence[0].summary).toContain('bun test');
    expect(version.evidence.some((item) => item.toolName === 'read_file')).toBe(false);
    expect(version.sections.find((section) => section.id === 'verification')?.items.join('\n')).toContain('278 tests passed');
    expect(artifactCopyPrompt(artifact)).toContain('Tool evidence:');
    expect(artifactCopyPrompt(artifact)).toContain('278 tests passed');
  });

  test('extracts code diffs from tool output into the artifact dashboard and copy prompt', () => {
    const diff = [
      'diff --git a/frontend/src/App.tsx b/frontend/src/App.tsx',
      '--- a/frontend/src/App.tsx',
      '+++ b/frontend/src/App.tsx',
      '@@ -10,6 +10,7 @@',
      " const title = 'Artifacts';",
      '-const showRawTranscript = true;',
      '+const showDiff = true;',
      '+const showRawTranscript = false;',
    ].join('\n');
    const artifact = buildSessionArtifact({
      session: { id: 'diff1', title: 'Show artifact diff' },
      messages: [
        { id: 'u1', role: 'user', content: 'Make the board show diffs.' },
        { id: 'patch', role: 'tool', toolName: 'patch', content: JSON.stringify({ tool_name: 'patch', success: true, diff }) },
        { id: 'a1', role: 'assistant', content: 'Added a diff panel to the artifact dashboard.' },
      ],
      now: 1710000400000,
    });
    const version = artifact.versions[0];

    expect(version.diffs).toHaveLength(1);
    expect(version.diffs[0]).toMatchObject({ file: 'frontend/src/App.tsx', added: 2, removed: 1 });
    expect(version.diffs[0].excerpt).toContain('+const showDiff = true;');
    expect(version.sections.find((section) => section.id === 'changes')?.items.join('\n')).toContain('frontend/src/App.tsx: +2 / -1');
    expect(artifactCopyPrompt(artifact)).toContain('Code diff:');
    expect(artifactCopyPrompt(artifact)).toContain('frontend/src/App.tsx (+2 / -1)');
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
