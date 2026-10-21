import type { CSSProperties, RefObject } from 'react';

interface QuickNoteToolsPanelProps {
  panelRef: RefObject<HTMLDivElement | null>;
  /** Ghost mode is on when opacity is 0, so the label flips to "restore". */
  isGhostMode: boolean;
  helpItems: string[];
  onSwitchNote: () => void;
  onToggleGhost: () => void;
  onCloseCurrent: () => void;
  onCloseAll: () => void;
  onToggleAllVisibility: () => void;
  onNewAndSwitch: () => void;
}

const PANEL_STYLE: CSSProperties = {
  position: 'absolute',
  top: 38,
  left: 14,
  right: 14,
  zIndex: 18,
  borderRadius: 18,
  border: '1px solid var(--quick-border)',
  background: 'var(--quick-surface)',
  boxShadow: 'var(--quick-shadow)',
  backdropFilter: 'blur(18px)',
  padding: 12,
};

const ACTION_STYLE: CSSProperties = {
  borderRadius: 14,
  border: '1px solid var(--quick-border)',
  background: 'var(--quick-surface-raised)',
  padding: '10px 12px',
  textAlign: 'left',
  fontSize: 12,
  color: 'var(--quick-text)',
  cursor: 'pointer',
};

/** Tools and shortcut cheat-sheet overlay of the quick-note window. */
export function QuickNoteToolsPanel({
  panelRef,
  isGhostMode,
  helpItems,
  onSwitchNote,
  onToggleGhost,
  onCloseCurrent,
  onCloseAll,
  onToggleAllVisibility,
  onNewAndSwitch,
}: QuickNoteToolsPanelProps) {
  const actions = [
    { label: '切换笔记', action: onSwitchNote },
    { label: isGhostMode ? '恢复显示' : '隐藏为 0%', action: onToggleGhost },
    { label: '关闭当前', action: onCloseCurrent },
    { label: '关闭全部', action: onCloseAll },
    { label: '最小化/恢复全部', action: onToggleAllVisibility },
    { label: '新建并切换', action: onNewAndSwitch },
  ];

  return (
    <div ref={panelRef} style={PANEL_STYLE}>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 8 }}>
        {actions.map((item) => (
          <button key={item.label} onClick={item.action} style={ACTION_STYLE}>
            {item.label}
          </button>
        ))}
      </div>
      <div
        style={{
          marginTop: 10,
          borderTop: '1px solid var(--quick-border)',
          paddingTop: 10,
          display: 'flex',
          flexWrap: 'wrap',
          gap: 6,
        }}
      >
        {helpItems.map((item) => (
          <span
            key={item}
            style={{
              borderRadius: 999,
              background: 'var(--quick-accent-soft)',
              padding: '4px 8px',
              fontSize: 11,
              color: 'var(--quick-muted)',
            }}
          >
            {item}
          </span>
        ))}
      </div>
    </div>
  );
}
