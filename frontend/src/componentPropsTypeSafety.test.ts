import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

const typedComponentBoundaries = [
  'ChatMain',
  'WorkspaceAside',
  'WorkspaceMain',
  'WorkspaceSidebar',
  'SkillMain',
  'WorkspaceEditorPreview',
  'WorkspaceBrowser',
];

describe('App component type boundaries', () => {
  test('page-level components do not accept any-typed props', () => {
    for (const component of typedComponentBoundaries) {
      const signature = source.match(new RegExp(`function ${component}\\(([^\\n]*)`))?.[1] || '';
      expect(signature).not.toMatch(/\bany\b/);
    }
  });

  test('shared workspace and chat contracts are named and reusable', () => {
    expect(source).toContain('type ChatMainProps = {');
    expect(source).toContain('type WorkspaceTreeProps = {');
    expect(source).toContain('type WorkspaceEditorPreviewProps = {');
    expect(source).not.toContain('useState<any[]>([])');
  });
});
