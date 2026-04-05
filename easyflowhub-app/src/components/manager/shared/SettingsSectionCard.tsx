import type { ReactNode } from 'react';

interface SettingsSectionCardProps {
  kicker: string;
  title: string;
  description: string;
  children: ReactNode;
  className?: string;
}

export function SettingsSectionCard(props: SettingsSectionCardProps) {
  const { kicker, title, description, children, className } = props;

  return (
    <section className={`rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5 ${className ?? ''}`.trim()}>
      <div className="mb-4">
        <p className="manager-kicker">{kicker}</p>
        <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">{title}</h3>
      </div>
      <p className="mb-4 text-sm text-[color:var(--manager-ink-soft)]">
        {description}
      </p>
      {children}
    </section>
  );
}
