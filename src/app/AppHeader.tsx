export function AppHeader() {
  return (
    <header className="shrink-0 flex items-end justify-between px-6 py-4 border-b border-zinc-800/80 bg-zinc-950/80">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">
          <span className="text-amber-400">RO</span>
          <span className="text-zinc-100">-Launcher</span>
        </h1>
        <p className="text-xs text-zinc-500 mt-0.5">Ragnarok Online · Linux</p>
      </div>
      <p className="text-[11px] text-zinc-600 tracking-wide">
        Developed by:{' '}
        <span className="text-zinc-400 font-medium">nndsk</span>
      </p>
    </header>
  )
}
