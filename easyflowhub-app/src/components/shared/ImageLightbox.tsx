import { useEffect } from 'react';

interface ImageLightboxProps {
  src: string;
  alt: string;
  onClose: () => void;
}

export function ImageLightbox({ src, alt, onClose }: ImageLightboxProps) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={alt || '图片预览'}
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 2000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 24,
        background: 'rgba(24, 21, 16, 0.72)',
        backdropFilter: 'blur(10px)',
      }}
    >
      <img
        src={src}
        alt={alt}
        onClick={(event) => event.stopPropagation()}
        style={{
          maxWidth: 'min(96vw, 1280px)',
          maxHeight: '90vh',
          borderRadius: 18,
          boxShadow: '0 24px 64px rgba(0, 0, 0, 0.32)',
          background: 'rgba(255, 255, 255, 0.92)',
          objectFit: 'contain',
        }}
      />
    </div>
  );
}
