import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

export type MacContextMenuItem =
  | {
      kind?: 'item';
      id: string;
      label: string;
      icon?: ReactNode;
      destructive?: boolean;
      disabled?: boolean;
      onSelect: () => void;
    }
  | {
      kind: 'separator';
      id: string;
    };

interface MacContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  items: MacContextMenuItem[];
  onClose: () => void;
}

export function MacContextMenu({
  open,
  x,
  y,
  items,
  onClose,
}: MacContextMenuProps) {
  const menuRef = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  useLayoutEffect(() => {
    if (!open || !menuRef.current) {
      setPos({ left: x, top: y });
      return;
    }
    const rect = menuRef.current.getBoundingClientRect();
    const pad = 8;
    const clampedX = Math.min(x, window.innerWidth - rect.width - pad);
    const clampedY = Math.min(y, window.innerHeight - rect.height - pad);
    setPos({
      left: Math.max(pad, clampedX),
      top: Math.max(pad, clampedY),
    });
  }, [open, x, y, items.length]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const handlePointerDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    const handleDismiss = () => onClose();

    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    window.addEventListener('blur', handleDismiss);
    window.addEventListener('scroll', handleDismiss, true);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('blur', handleDismiss);
      window.removeEventListener('scroll', handleDismiss, true);
    };
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  return (
    <div
      ref={menuRef}
      role="menu"
      style={{ left: pos.left, top: pos.top }}
      className="fixed z-50 min-w-[186px] rounded-[8px] bg-[var(--bg-popover)] p-1 text-[12.5px] text-[var(--text-primary)] shadow-[0_12px_32px_rgba(0,0,0,0.24),0_0_0_0.5px_var(--hairline)] backdrop-blur-2xl"
    >
      {items.map((item) => {
        if (item.kind === 'separator') {
          return (
            <div
              key={item.id}
              role="separator"
              className="mx-1.5 my-1 h-[1px] bg-[var(--hairline-inset)]"
            />
          );
        }
        return (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onSelect();
            }}
            className={[
              'flex w-full items-center gap-2 rounded-[5px] px-2.5 py-1 text-left transition-colors duration-100 disabled:cursor-not-allowed disabled:opacity-45',
              item.destructive
                ? 'text-[var(--status-red)] hover:bg-[var(--status-red)] hover:text-white'
                : 'hover:bg-[var(--accent)] hover:text-white',
            ].join(' ')}
          >
            {item.icon && (
              <span className="inline-flex h-3.5 w-3.5 shrink-0 items-center justify-center opacity-85">
                {item.icon}
              </span>
            )}
            <span className="truncate">{item.label}</span>
          </button>
        );
      })}
    </div>
  );
}
