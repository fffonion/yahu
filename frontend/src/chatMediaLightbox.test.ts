import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const markdown = () => readFileSync(new URL('./markdown.ts', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat media image lightbox', () => {
  test('chat markdown image media carries clickable lightbox metadata', () => {
    const source = markdown();
    expect(source).toContain('export function chatMediaImagesFromMarkdown');
    expect(source).toContain('class="md-media-open"');
    expect(source).toContain('data-chat-image-path=');
    expect(source).toContain('data-chat-image-src=');
    expect(source).toContain('data-chat-image-name=');
  });

  test('chat page opens images in the gallery-style modal without delete controls', () => {
    const source = app();
    expect(source).toContain('chatMediaImagesFromMarkdown');
    expect(source).toContain('function ChatImageLightbox(');
    expect(source).toContain('className={`image-modal chat-image-modal');
    expect(source).toContain('onClick={onChatMediaClick}');
    expect(source).toContain('aria-label={t(\'gallery.download\')}');
    expect(source).toContain('aria-label={t(\'gallery.metadata\')}');
    expect(source).toContain('aria-label={t(\'gallery.previous\')}');
    expect(source).toContain('aria-label={t(\'gallery.next\')}');

  });

  test('chat page opens HTML media in an isolated full-screen iframe preview', () => {
    const source = app();
    expect(source).toContain('chatMediaHtmlsFromMarkdown');
    expect(source).toContain('a.md-media-html-open');
    expect(source).toContain('function ChatHtmlPreview(');
    expect(source).toContain('className="html-media-modal"');
    expect(source).toContain('className="html-media-preview-frame"');
    expect(source).toContain('sandbox="allow-scripts"');
    expect(source).toContain('aria-modal="true"');
    expect(source).toContain('if (event.key === \'Escape\') onClose()');
    const styles = css();
    expect(styles).toContain('.html-media-modal{position:fixed;inset:0;z-index:260');
    expect(styles).toContain('.html-media-preview-frame{width:100%;height:100%;min-height:0;border:0');
  });

  test('chat lightbox reuses image modal styling and marks chat-specific scope', () => {
    const styles = css();
    expect(styles).toContain('.chat-image-modal{z-index:220}.chat-image-modal .modal-meta{z-index:238}.chat-image-modal .modalbar{z-index:240}.chat-image-modal .modalbar button.danger{display:none!important}');
    expect(styles).toContain('.msg-body .md-media-open{display:block;cursor:zoom-in}');
  });

  test('chat lightbox supports mobile pinch zoom with the same pointer model as gallery', () => {
    const source = app();
    expect(source).toContain('chatPointers');
    expect(source).toContain('chatPinchStart');
    expect(source).toContain('beginChatPinch');
    expect(source).toContain('chatPointers.current.size >= 2');
    expect(source).toContain('zoom.current.scale = clampNumber(start.scale * distance / start.distance, 1, 6)');
    const styles = css();
    expect(styles).toContain('.image-modal{touch-action:none');
  });
});
