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
    expect(source).toContain('aria-label="Download image"');
    expect(source).toContain('aria-label="Metadata"');
    expect(source).toContain('aria-label="Previous image"');
    expect(source).toContain('aria-label="Next image"');
    expect(source).not.toContain('chat-image-modal-delete');
    expect(source).not.toContain('aria-label="Delete chat image"');
  });

  test('chat lightbox reuses image modal styling and marks chat-specific scope', () => {
    const styles = css();
    expect(styles).toContain('.chat-image-modal{z-index:220}.chat-image-modal .modal-meta{z-index:238}.chat-image-modal .modalbar{z-index:240}.chat-image-modal .modalbar button.danger{display:none!important}');
    expect(styles).toContain('.msg-body .md-media-open{display:block;cursor:zoom-in}');
  });
});
