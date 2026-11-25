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
        'inline-flex h-7 w-9 shrink-0 items-center justify-center rounded-md transition-transform duration-120 active:scale-[0.95]',
        disabled ? 'cursor-not-allowed opacity-65' : 'cursor-pointer',
      ].join(' ')}
    >
      <span
        aria-hidden="true"
        className={[
          'relative inline-flex h-[18px] w-[32px] items-center rounded-full p-[2px] shadow-[inset_0_1px_2px_rgba(0,0,0,0.16)] transition-colors duration-150',
          checked
            ? 'bg-[var(--accent)]'
            : 'bg-black/18 dark:bg-white/22',
        ].join(' ')}
      >
        <span
          className={[
            'h-[14px] w-[14px] rounded-full bg-white shadow-[0_1px_2.5px_rgba(0,0,0,0.28)] transition-transform duration-150 ease-[cubic-bezier(0.16,1,0.3,1)]',
            checked ? 'translate-x-[14px]' : 'translate-x-0',
          ].join(' ')}
        />
      </span>
    </button>
  );
}
