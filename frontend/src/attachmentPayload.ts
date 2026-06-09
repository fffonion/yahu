export type AttachmentForPayload = {
  name: string;
  kind: 'image' | 'text' | 'binary';
  mime: string;
  size: number;
  dataUrl?: string;
  text?: string;
  uploadedPath?: string;
};

const fmtSize = (bytes?: number) => bytes === undefined ? '' : bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} k` : `${(bytes / 1024 / 1024).toFixed(1)} M`;

export function buildChatInputWithAttachments(input: string, attachments: AttachmentForPayload[]): unknown {
  const textParts = [input.trim()];
  for (const att of attachments) {
    const savedAt = att.uploadedPath ? `\nSaved path: ${att.uploadedPath}` : '';
    if (att.kind === 'image') {
      textParts.push(`\n\nAttached image: ${att.name} (${fmtSize(att.size)}, ${att.mime}).${savedAt}`);
    } else if (att.kind === 'text') {
      textParts.push(`\n\nAttached text file: ${att.name} (${fmtSize(att.size)}, ${att.mime}).${savedAt}\n\n\`\`\`\n${att.text || ''}\n\`\`\``);
    } else {
      textParts.push(`\n\nAttached file: ${att.name} (${fmtSize(att.size)}, ${att.mime}).${savedAt}\nUse the saved path if you need to inspect or process the file.`);
    }
  }
  const text = textParts.join('\n').trim();
  const images = attachments.filter((a) => a.kind === 'image' && a.dataUrl);
  return images.length ? [{ type: 'text', text }, ...images.map((a) => ({ type: 'image_url', image_url: { url: a.dataUrl, detail: 'high' } }))] : text;
}
