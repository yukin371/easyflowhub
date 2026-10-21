import type { CSSProperties } from 'react';

interface QuickNoteImageStripProps {
  /** Resolved asset URLs for the note's attached images, in attachment order. */
  images: { src: string; alt: string }[];
  /** Filenames used as stable keys; may be shorter than `images`. */
  filenames: (string | undefined)[];
  onOpen: (image: { src: string; alt: string }) => void;
  onRemove: (index: number) => void;
}

const STRIP_STYLE: CSSProperties = {
  flex: '0 0 auto',
  display: 'flex',
  gap: 8,
  overflowX: 'auto',
  paddingInline: 4,
  paddingBottom: 2,
};

const THUMB_STYLE: CSSProperties = {
  position: 'relative',
  width: 72,
  minWidth: 72,
  height: 72,
  borderRadius: 14,
  overflow: 'hidden',
  border: '1px solid var(--quick-border)',
  background: 'var(--quick-surface-raised)',
  boxShadow: '0 6px 18px color-mix(in srgb, var(--quick-text) 12%, transparent)',
};

const REMOVE_STYLE: CSSProperties = {
  position: 'absolute',
  right: 4,
  top: 4,
  width: 18,
  height: 18,
  borderRadius: 999,
  border: 'none',
  background: 'var(--quick-surface)',
  color: 'var(--quick-text)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  cursor: 'pointer',
  backdropFilter: 'blur(6px)',
};

/**
 * Thumbnail strip for images attached to the quick-note draft: double click
 * opens the lightbox, the corner button detaches the attachment.
 */
export function QuickNoteImageStrip({ images, filenames, onOpen, onRemove }: QuickNoteImageStripProps) {
  return (
    <div style={STRIP_STYLE}>
      {images.map(({ src, alt }, index) => (
        <div key={filenames[index] ?? `${src}-${index}`} style={THUMB_STYLE}>
          <img
            src={src}
            alt={alt || `插入图片 ${index + 1}`}
            title="双击放大"
            onDoubleClick={() => onOpen({ src, alt })}
            style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block', cursor: 'zoom-in' }}
          />
          <button onClick={() => onRemove(index)} title="移除图片" style={REMOVE_STYLE}>
            <span style={{ fontSize: 11, lineHeight: 1 }}>×</span>
          </button>
        </div>
      ))}
    </div>
  );
}
