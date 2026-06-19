import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('artifacts UI', () => {
  test('adds artifacts as a first-class route and navigation mode', () => {
    const source = app();
    expect(source).toContain("type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'artifacts' | 'images' | 'workspace' | 'skills' | 'settings';");
    expect(source).toContain("nav-artifacts");
    expect(source).toContain("setNavMode('artifacts', true)");
    expect(source).toContain("{mode === 'artifacts' && <ArtifactsMain");
    expect(i18n()).toContain("'nav.artifacts'");
  });

  test('chat header can publish the current session into the artifacts gallery', () => {
    const source = app();
    expect(source).toContain('createSessionArtifact={createSessionArtifact}');
    expect(source).toContain("aria-label={t('artifacts.createFromSession')}");
    expect(source).toContain('buildSessionArtifact({');
    expect(source).toContain('localStorage.setItem(ARTIFACTS_KEY');
  });

  test('artifacts page uses a gallery and Claude-inspired document surface', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function ArtifactsMain');
    expect(source).toContain('artifact-gallery');
    expect(source).toContain('artifact-document');
    expect(source).toContain('copyTextToClipboard(artifactCopyPrompt(selected, activeVersion))');
    expect(source).toContain('artifact-version-help');
    expect(i18n()).toContain("'artifacts.versionHelp'");
    expect(source).toContain('artifact-evidence-grid');
    expect(source).toContain('activeVersion.evidence');
    expect(source).toContain('activeVersion.sections');
    expect(i18n()).toContain("'artifacts.toolEvidence'");
    expect(styles).toContain('.artifact-evidence-card');
    expect(styles).toContain('.artifacts-main');
    expect(styles).toContain('.artifact-document');
    expect(styles).toContain('#f5f4ed');
    expect(styles).toContain('#c96442');
  });
});
