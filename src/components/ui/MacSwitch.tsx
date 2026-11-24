interface MacSwitchProps {
  checked: boolean;
  disabled?: boolean;
  label: string;
  tooltip?: string;
  onChange?: (next: boolean) => void;
}

export function MacSwitch({
  checked,
  disabled = false,
  label,
  tooltip,
  onChange,
}: MacSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      title={tooltip}
      onClick={() => onChange?.(!checked)}
      className={[
        'inline-flex h-7 w-9 shrink-0 items-center justify-center rounded-md',
        disabled ? 'cursor-not-allowed opacity-75' : 'cursor-pointer',
      ].join(' ')}
    >
      <span
        aria-hidden="true"
        className={[
          'relative inline-flex h-[17px] w-[30px] items-center rounded-full p-[2px] transition-opacity duration-150',
          checked
            ? 'bg-[var(--accent)]'
            : 'bg-black/18 dark:bg-white/20',
        ].join(' ')}
      >
        <span
          className={[
            'h-[13px] w-[13px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.22)] transition-transform duration-150 ease-[cubic-bezier(0.16,1,0.3,1)]',
            checked ? 'translate-x-[13px]' : 'translate-x-0',
          ].join(' ')}
        />
      </span>
    </button>
  );
}
