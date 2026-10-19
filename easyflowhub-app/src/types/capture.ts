export type CaptureKind = 'text' | 'link' | 'image' | 'file' | 'audio' | 'transcript';
export type CaptureAttachmentKind = CaptureKind;

export interface CaptureAttachmentBase<Kind extends CaptureAttachmentKind> {
  id: string;
  kind: Kind;
  name?: string;
  source: string;
  createdAt: string;
  /** Null until the capture is persisted into the Inbox. */
  inboxItemId: string | null;
}

export type CaptureAttachmentLocation =
  | { kind: 'local-path'; value: string }
  | { kind: 'data-url'; value: string };

export type CaptureAttachment =
  | (CaptureAttachmentBase<'image' | 'file' | 'audio'> & {
      mimeType: string;
      sizeBytes?: number;
      location: CaptureAttachmentLocation;
    })
  | (CaptureAttachmentBase<'link'> & { url: string })
  | (CaptureAttachmentBase<'text'> & { content: string; mimeType?: string })
  | (CaptureAttachmentBase<'transcript'> & {
      content: string;
      mimeType?: string;
      audioAttachmentId?: string;
    });

/** Normalized content envelope before it is written into an InboxItem. */
export interface Capture {
  id: string;
  kind: CaptureKind;
  title?: string;
  content?: string;
  source: string;
  context?: string;
  capturedAt: string;
  attachments: CaptureAttachment[];
}
