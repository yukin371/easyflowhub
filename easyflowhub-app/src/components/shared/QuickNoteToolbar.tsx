import type { CSSProperties, RefObject } from 'react';

interface QuickNoteToolbarProps {
  /** Rounded opacity percent; 0 renders the ghost label. */
  opacity: number;
  dragBackground: string;
  alwaysOnTop: boolean;
  toolsButtonRef: RefObject<HTMLButtonElement | null>;
  onToggleTools: () => void;
  onToggleAlwaysOnTop: () => void;
}

function alphaAtLeast(color: string, minimum: number, percentage: number): string {
  const ratio = Math.max(minimum, percentage);
  return `color-mix(in srgb, ${color} ${Math.round(ratio * 100)}%, transparent)`;
}

/** Draggable title strip of the quick-note window with the tools and pin buttons. */
export function QuickNoteToolbar({
  opacity,
  dragBackground,
  alwaysOnTop,
  toolsButtonRef,
  onToggleTools,
  onToggleAlwaysOnTop,
}: QuickNoteToolbarProps) {
  const isGhostMode = opacity <= 0.01;
  const stripStyle: CSSProperties = {
    position: 'relative',
    height: 32,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingInline: 8,
    backgroundColor: dragBackground,
    flexShrink: 0,
  };

  return (
    <div data-tauri-drag-region style={stripStyle}>
      <div
        style={{
          paddingInline: 8,
          fontSize: 10,
          letterSpacing: '0.22em',
          color: alphaAtLeast('var(--quick-muted)', 0.26, opacity * 0.72),
        }}
      >
        {isGhostMode ? 'GHOST' : `${Math.round(opacity * 100)}%`}
      </div>
      <button
        ref={toolsButtonRef}
        onClick={onToggleTools}
        title="工具与快捷键"
        style={{
          position: 'absolute',
          left: '50%',
          transform: 'translateX(-50%)',
          width: 20,
          height: 20,
          borderRadius: 999,
          border: 'none',
          background: 'transparent',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: alphaAtLeast('var(--quick-text)', 0.32, opacity * 0.72),
          cursor: 'pointer',
        }}
      >
        <span style={{ fontSize: 12, lineHeight: 1, marginTop: -2, letterSpacing: '0.06em' }}>•••</span>
      </button>
      <button
        onClick={onToggleAlwaysOnTop}
        style={{
          width: 28,
          height: 28,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          border: 'none',
          background: 'none',
          cursor: 'pointer',
          color: alwaysOnTop ? 'var(--quick-accent)' : 'var(--quick-muted)',
        }}
        title={alwaysOnTop ? '取消置顶' : '置顶'}
      >
        <svg width="16" height="16" fill={alwaysOnTop ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
        </svg>
      </button>
    </div>
  );
}
