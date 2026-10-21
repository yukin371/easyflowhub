import type { CSSProperties } from 'react';
import { buildNoteSwitchLabel, isNoteContinueCandidate } from '../../lib/notesUtils';
import type { Note } from '../../types/note';

interface NoteSwitcherPanelProps {
  /** Current draft query, owned by the page so it can reset on toggle. */
  query: string;
  onQueryChange: (query: string) => void;
  /** 0 is the "new note" row; 1..n map to the filtered recent notes. */
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  notes: Note[];
  currentNoteId: string;
  inputRef: React.RefObject<HTMLInputElement | null>;
  onCreateAndSwitch: () => void;
  onSwitchNote: (id: string) => void;
}

const PANEL_STYLE: CSSProperties = {
  position: 'absolute',
  top: 44,
  left: 18,
  right: 18,
  zIndex: 20,
  borderRadius: 18,
  border: '1px solid var(--quick-border)',
  background: 'var(--quick-surface)',
  boxShadow: 'var(--quick-shadow)',
  backdropFilter: 'blur(20px)',
  padding: 12,
};

const INPUT_STYLE: CSSProperties = {
  width: '100%',
  border: '1px solid var(--quick-border)',
  borderRadius: 14,
  background: 'var(--quick-input)',
  padding: '10px 12px',
  fontSize: 13,
  color: 'var(--quick-text)',
  outline: 'none',
};

function rowStyle(isSelected: boolean, isCurrent: boolean): CSSProperties {
  const highlighted = isSelected || isCurrent;
  return {
    border: `1px solid var(--quick-${highlighted ? 'accent' : 'border'})`,
    background: highlighted ? 'var(--quick-accent-soft)' : 'var(--quick-surface-raised)',
    borderRadius: 14,
    padding: '10px 12px',
    textAlign: 'left',
    color: 'var(--quick-text)',
    cursor: 'pointer',
  };
}

/**
 * Quick-note note switcher overlay: query input plus the "new note" row and the
 * filtered recent notes. Selection state stays in the page because the window
 * keyboard map drives it; this component is presentation plus arrow/Enter keys.
 */
export function NoteSwitcherPanel({
  query,
  onQueryChange,
  selectedIndex,
  onSelectIndex,
  notes,
  currentNoteId,
  inputRef,
  onCreateAndSwitch,
  onSwitchNote,
}: NoteSwitcherPanelProps) {
  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      onSelectIndex(Math.min(selectedIndex + 1, notes.length));
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      onSelectIndex(Math.max(selectedIndex - 1, 0));
      return;
    }

    if (event.key !== 'Enter') {
      return;
    }

    event.preventDefault();
    const targetNote = notes[selectedIndex - 1];
    if (targetNote) {
      onSwitchNote(targetNote.id);
      return;
    }

    if (selectedIndex === 0) {
      onCreateAndSwitch();
    }
  };

  return (
    <div style={PANEL_STYLE}>
      <input
        ref={inputRef}
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="切换笔记，输入标题、内容或标签"
        style={INPUT_STYLE}
      />
      <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 220, overflowY: 'auto' }}>
        <button onClick={onCreateAndSwitch} style={rowStyle(selectedIndex === 0, false)}>
          新建笔记
        </button>
        {notes.map((item, index) => (
          <button
            key={item.id}
            onClick={() => onSwitchNote(item.id)}
            style={rowStyle(selectedIndex === index + 1, item.id === currentNoteId)}
          >
            <div style={{ fontSize: 13 }}>{buildNoteSwitchLabel(item)}</div>
            <div style={{ marginTop: 4, fontSize: 11, color: 'var(--quick-muted)' }}>
              {isNoteContinueCandidate(item) ? '继续编辑' : '空白笔记'}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
