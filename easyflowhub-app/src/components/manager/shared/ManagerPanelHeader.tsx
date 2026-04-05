import type { ReactNode } from 'react';

interface ManagerPanelHeaderProps {
  kicker: string;
  title: string;
  description?: string;
  actions?: ReactNode;
  className?: string;
}

export function ManagerPanelHeader(props: ManagerPanelHeaderProps) {
  const { kicker, title, description, actions, className } = props;

  return (
    <header className={className}>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="manager-kicker">{kicker}</p>
          <h2 className="mt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-2xl text-[color:var(--manager-ink-strong)]">
            {title}
          </h2>
          {description ? (
            <p className="mt-1 text-sm text-[color:var(--manager-ink-soft)]">
              {description}
            </p>
          ) : null}
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
      </div>
    </header>
  );
}

