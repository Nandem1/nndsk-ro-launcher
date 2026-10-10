import { WindowControls } from './WindowControls'

export function LoadingScreen() {
  return (
    <div className="h-full flex flex-col bg-surface">
      <div
        className="shrink-0 flex justify-end px-2 py-1"
        data-tauri-drag-region
      >
        <WindowControls />
      </div>
      <div className="flex-1 flex flex-col items-center justify-center gap-4">
        <span className="text-brand font-sans font-semibold tracking-brand text-ink">
          RO LAUNCHER
        </span>
        <div className="w-8 h-8 rounded-pill border-2 border-line border-t-muted animate-pulse-dot" />
      </div>
    </div>
  )
}
