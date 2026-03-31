import { useEffect, useState } from 'react';

interface InlineTitleEditorProps {
  title: string;
  placeholder: string;
  className: string;
  titleClassName?: string;
  inputClassName?: string;
  onOpen?: () => void;
  onSave: (value: string) => void | Promise<void>;
}

export function InlineTitleEditor(props: InlineTitleEditorProps) {
  const {
    title,
    placeholder,
    className,
    titleClassName,
    inputClassName,
    onOpen,
    onSave,
  } = props;
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(title);

  useEffect(() => {
    if (!isEditing) {
      setDraft(title);
    }
  }, [isEditing, title]);

  if (isEditing) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onClick={(event) => event.stopPropagation()}
        onBlur={() => {
          setIsEditing(false);
          void onSave(draft);
        }}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key === 'Enter') {
            event.preventDefault();
            setIsEditing(false);
            void onSave(draft);
          }
          if (event.key === 'Escape') {
            event.preventDefault();
            setDraft(title);
            setIsEditing(false);
          }
        }}
        placeholder={placeholder}
        className={inputClassName ?? className}
      />
    );
  }

  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        setIsEditing(true);
      }}
      onDoubleClick={(event) => {
        event.stopPropagation();
        onOpen?.();
      }}
      className={className}
      title="点击修改标题，双击进入笔记"
    >
      <span className={titleClassName}>{title || placeholder}</span>
    </button>
  );
}
