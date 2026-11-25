import type { ChangeEvent, ReactNode } from 'react';
import { ChevronsUpDown } from 'lucide-react';

interface MacSelectProps {
  id?: string;
  value: string | number;
  onChange: (e: ChangeEvent<HTMLSelectElement>) => void;
  'aria-label'?: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  className?: string;
  children: ReactNode;
}

export function MacSelect({
  id,
  value,
  onChange,
  'aria-label': ariaLabel,
  disabled = false,
  size = 'md',
  className = '',
  children,
}: MacSelectProps) {
  return (
    <div className={['relative inline-flex items-center', className].join(' ')}>
      <select
        id={id}
        value={value}
        onChange={onChange}
        aria-label={ariaLabel}
        disabled={disabled}
        className={[
          'mac-btn w-full cursor-pointer appearance-none bg-[var(--bg-control)] pr-6 pl-2.5 text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-55',
          size === 'sm' ? 'h-7 text-[12px]' : 'h-8 text-[12.5px]',
        ].join(' ')}
      >
        {children}
      </select>
      <ChevronsUpDown
        className="pointer-events-none absolute right-2 h-3 w-3 text-[var(--text-secondary)]"
        strokeWidth={2}
        aria-hidden="true"
      />
    </div>
  );
}
