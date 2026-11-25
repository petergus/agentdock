export function SkeletonLoader() {
  return (
    <div className="flex h-full w-full overflow-hidden" aria-busy="true" aria-label="Scanning workspace">
      <aside className="flex w-[256px] shrink-0 flex-col border-r border-[var(--hairline)] px-3 pt-12 pb-3">
        <div className="h-7 w-full rounded-[7px] bg-[var(--bg-field)]" />
        <div className="mt-4 h-8 w-full rounded-[6px] bg-[var(--bg-subtle)]" />
        <div className="mt-5 space-y-2">
          <div className="h-3 w-20 rounded bg-[var(--bg-subtle)]" />
          <div className="h-7 w-full rounded-[6px] bg-[var(--bg-subtle)]" />
          <div className="h-7 w-full rounded-[6px] bg-[var(--bg-subtle)]" />
          <div className="h-7 w-4/5 rounded-[6px] bg-[var(--bg-subtle)]" />
        </div>
      </aside>
      <main className="flex-1 bg-[var(--bg-canvas)]">
        <div className="border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-6 pt-3 pb-3.5">
          <div className="flex items-center justify-between">
            <div className="h-6 w-44 rounded bg-[var(--bg-subtle)]" />
            <div className="h-7 w-64 rounded-[7px] bg-[var(--bg-subtle)]" />
          </div>
          <div className="mt-4 grid grid-cols-2 gap-5">
            <div className="h-8 rounded bg-[var(--bg-subtle)]" />
            <div className="h-8 rounded bg-[var(--bg-subtle)]" />
          </div>
        </div>
        <div className="mx-auto max-w-[960px] space-y-3 px-6 py-5">
          <div className="h-4 w-28 rounded bg-[var(--bg-subtle)]" />
          <div className="mac-group space-y-2 p-3">
            <div className="h-7 w-full rounded bg-[var(--bg-subtle)]" />
            <div className="h-7 w-full rounded bg-[var(--bg-subtle)]" />
            <div className="h-7 w-full rounded bg-[var(--bg-subtle)]" />
          </div>
        </div>
      </main>
    </div>
  );
}
