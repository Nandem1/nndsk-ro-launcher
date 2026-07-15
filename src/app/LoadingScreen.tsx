export function LoadingScreen() {
  return (
    <div className="h-full flex flex-col items-center justify-center gap-4 bg-zinc-950">
      <span className="text-xl font-bold tracking-widest text-zinc-200">
        RO LAUNCHER
      </span>
      <div className="w-8 h-8 rounded-full border-2 border-zinc-700 border-t-amber-500 animate-spin" />
    </div>
  )
}
