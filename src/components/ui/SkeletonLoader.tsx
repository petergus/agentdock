export function SkeletonLoader() {
  return (
    <div className="flex h-full w-full overflow-hidden" aria-busy="true" aria-label="Scanning workspace">
      <aside className="flex w-[260px] shrink-0 flex-col border-r border-[var(--hairline)] px-3 pt-11 pb-3">
        <div className="h-7 w-full rounded-md bg-[var(--bg-subtle)]" />
        <div className="mt-4 h-8 w-full rounded-md bg-[var(--bg-subtle)]" />
        <div className="mt-5 space-y-2">
          <div className="h-3 w-20 rounded bg-[var(--bg-subtle)]" />
          <div className="h-7 w-full rounded-md bg-[var(--bg-subtle)]" />
          <div className="h-7 w-full rounded-md bg-[var(--bg-subtle)]" />
          <div className="h-7 w-4/5 rounded-md bg-[var(--bg-subtle)]" />
        </div>
      </aside>
      <main className="flex-1 bg-[var(--bg-content)] p-6">
        <div className="flex items-center justify-between">
          <div className="h-6 w-44 rounded bg-[var(--bg-subtle)]" />
          <div className="h-7 w-64 rounded-md bg-[var(--bg-subtle)]" />
        </div>
        <div className="mt-5 grid grid-cols-2 gap-5">
          <div className="h-10 rounded bg-[var(--bg-subtle)]" />
          <div className="h-10 rounded bg-[var(--bg-subtle)]" />
        </div>
        <div className="mt-8 space-y-2.5">
          <div className="h-4 w-28 rounded bg-[var(--bg-subtle)]" />
          <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
          <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
          <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
          <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
        </div>
      </main>
    </div>
  );
}
