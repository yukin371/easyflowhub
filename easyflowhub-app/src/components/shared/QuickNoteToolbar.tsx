import type { CSSProperties, RefObject } from 'react';

interface QuickNoteToolbarProps {
  /** Rounded opacity percent; 0 renders the ghost label. */
  opacity: number;
  dragBackground: string;
  alwaysOnTop: boolean;
  toolsButtonRef: RefObject<HTMLButtonElement | null>;
  onToggleTools: () => void;
  onToggleAlwaysOnTop: () => void;
  /** 预览态下正文改为渲染后的图文混排，按钮翻转为「回到编辑」。 */
  previewMode: boolean;
  onTogglePreview: () => void;
}

function alphaAtLeast(color: string, minimum: number, percentage: number): string {
  const ratio = Math.max(minimum, percentage);
  return `color-mix(in srgb, ${color} ${Math.round(ratio * 100)}%, transparent)`;
}

const STRIP_STYLE: CSSProperties = {
  height: 32,
  display: 'flex',
  alignItems: 'center',
  // Three fixed slots. The tools button used to be absolutely centred while the
  // preview/pin buttons stayed in the flex flow, so on a narrow window the dots
  // landed on top of the eye icon. A real centre track cannot overlap a sibling.
  gap: 4,
  paddingInline: 8,
  flexShrink: 0,
};

const SLOT_STYLE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  flexShrink: 0,
};

const ICON_BUTTON_STYLE: CSSProperties = {
  width: 28,
  height: 28,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: 'none',
  background: 'none',
  cursor: 'pointer',
  borderRadius: 8,
};

/** Draggable title strip of the quick-note window with the tools and pin buttons. */
export function QuickNoteToolbar({
  opacity,
  dragBackground,
  alwaysOnTop,
  toolsButtonRef,
  onToggleTools,
  onToggleAlwaysOnTop,
  previewMode,
  onTogglePreview,
}: QuickNoteToolbarProps) {
  const isGhostMode = opacity <= 0.01;
  const labelColor = alphaAtLeast('var(--quick-muted)', 0.26, opacity * 0.72);
  const toolsColor = alphaAtLeast('var(--quick-text)', 0.32, opacity * 0.72);

  return (
    <div data-tauri-drag-region style={{ ...STRIP_STYLE, position: 'relative', backgroundColor: dragBackground }}>
      <div
        style={{
          ...SLOT_STYLE,
          minWidth: 46,
          paddingInline: 8,
          fontSize: 10,
          letterSpacing: '0.22em',
          color: labelColor,
        }}
      >
        {isGhostMode ? 'GHOST' : `${Math.round(opacity * 100)}%`}
      </div>
      <div style={{ flex: 1, display: 'flex', justifyContent: 'center', minWidth: 0 }}>
        <button
          ref={toolsButtonRef}
          onClick={onToggleTools}
          title="工具与快捷键"
          aria-label="工具与快捷键"
          style={{
            width: 26,
            height: 20,
            borderRadius: 999,
            border: 'none',
            background: 'transparent',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: toolsColor,
            cursor: 'pointer',
          }}
        >
          <span style={{ fontSize: 12, lineHeight: 1, marginTop: -2, letterSpacing: '0.06em' }}>•••</span>
        </button>
      </div>
      <div style={{ ...SLOT_STYLE, gap: 2 }}>
        <button
          onClick={onTogglePreview}
          style={{
            ...ICON_BUTTON_STYLE,
            color: previewMode ? 'var(--quick-accent)' : 'var(--quick-muted)',
          }}
          title={previewMode ? '回到编辑' : '预览（图片在正文内联显示）'}
          aria-label={previewMode ? '回到编辑' : '预览'}
          aria-pressed={previewMode}
        >
          {previewMode ? (
            <svg width="15" height="15" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2v-5" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.5 2.5a2.12 2.12 0 013 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
          ) : (
            <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
              <circle cx="12" cy="12" r="3" strokeWidth={2} />
            </svg>
          )}
        </button>
        <button
          onClick={onToggleAlwaysOnTop}
          style={{
            ...ICON_BUTTON_STYLE,
            color: alwaysOnTop ? 'var(--quick-accent)' : 'var(--quick-muted)',
          }}
          title={alwaysOnTop ? '取消置顶' : '置顶'}
          aria-label={alwaysOnTop ? '取消置顶' : '置顶'}
          aria-pressed={alwaysOnTop}
        >
          <svg width="16" height="16" fill={alwaysOnTop ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
        </button>
      </div>
    </div>
  );
}
