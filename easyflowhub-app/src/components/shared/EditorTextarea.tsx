import type { CSSProperties, RefObject, TextareaHTMLAttributes } from 'react';
import type { EditorSettings } from '../../types/settings';

type EditorTextareaVariant = 'manager' | 'quick-note';

interface EditorTextareaProps
  extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'ref'> {
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  editorSettings: EditorSettings;
  variant: EditorTextareaVariant;
  dragActive?: boolean;
  dragHint?: string;
}

const BASE_STYLE: Record<EditorTextareaVariant, CSSProperties> = {
  manager: {},
  'quick-note': {
    flex: 1,
    width: '100%',
    height: '100%',
    padding: 18,
    border: 'none',
    outline: 'none',
    resize: 'none',
    backgroundColor: 'transparent',
    color: '#1a1a1a',
    fontSize: 14,
    lineHeight: 1.75,
    fontFamily: 'inherit',
  },
};

const BASE_CLASS_NAME: Record<EditorTextareaVariant, string> = {
  manager:
    'h-full w-full resize-none rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-5 py-4 text-[15px] leading-8 text-[color:var(--manager-ink)] outline-none placeholder:text-[color:var(--manager-ink-subtle)] sm:px-6 sm:py-5',
  'quick-note': '',
};

export function EditorTextarea(props: EditorTextareaProps) {
  const {
    textareaRef,
    editorSettings,
    variant,
    dragActive = false,
    dragHint = '拖入图片以插入',
    className,
    style,
    ...rest
  } = props;

  const caretClassName =
    variant === 'manager'
      ? `manager-editor manager-caret manager-caret--${editorSettings.cursor_style}${editorSettings.cursor_trail ? ' manager-caret--trail' : ''}`
      : `quick-note-editor quick-note-caret quick-note-caret--${editorSettings.cursor_style}${editorSettings.cursor_trail ? ' quick-note-caret--trail' : ''}`;

  return (
    <div
      style={{
        position: 'relative',
        flex: variant === 'quick-note' ? 1 : undefined,
        height: variant === 'manager' ? '100%' : undefined,
      }}
    >
      <textarea
        {...rest}
        ref={textareaRef}
        className={`${caretClassName} ${BASE_CLASS_NAME[variant]} ${className ?? ''}`.trim()}
        style={{
          ...BASE_STYLE[variant],
          caretColor: editorSettings.cursor_color,
          boxShadow:
            editorSettings.cursor_style === 'focus'
              ? `inset 0 0 0 1px color-mix(in srgb, var(--manager-accent) 10%, transparent), inset 0 0 28px color-mix(in srgb, var(--manager-accent) ${editorSettings.cursor_trail ? 7 : 3}%, transparent)`
              : editorSettings.cursor_style === 'accent'
                ? `inset 0 -20px 40px color-mix(in srgb, var(--manager-accent) ${editorSettings.cursor_trail ? 6 : 2}%, transparent)`
                : 'none',
          ...(dragActive
            ? {
                backgroundColor:
                  variant === 'manager' ? 'var(--manager-accent-soft)' : 'var(--quick-accent-soft)',
                outline: variant === 'manager' ? '2px dashed var(--manager-accent)' : '2px dashed var(--quick-accent)',
                outlineOffset: variant === 'manager' ? -8 : -6,
              }
            : null),
          ...style,
        }}
      />
      {dragActive ? (
        <div
          style={{
            position: 'absolute',
            inset: variant === 'manager' ? 12 : 8,
            pointerEvents: 'none',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: variant === 'manager' ? 16 : 12,
            border: variant === 'manager' ? '2px dashed var(--manager-accent)' : '2px dashed var(--quick-accent)',
            background: variant === 'manager' ? 'var(--manager-panel-strong)' : 'var(--quick-surface-raised)',
            color: variant === 'manager' ? 'var(--manager-accent)' : 'var(--quick-accent)',
            fontSize: 14,
            fontWeight: 600,
            letterSpacing: '0.08em',
          }}
        >
          {dragHint}
        </div>
      ) : null}
    </div>
  );
}
