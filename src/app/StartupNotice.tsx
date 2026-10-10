import { AlertTriangle, RefreshCw } from 'lucide-react'
import { Button } from '../shared/ui/Button'

interface StartupNoticeProps {
  errors: string[]
  retrying: boolean
  onRetry: () => void
}

export function StartupNotice({
  errors,
  retrying,
  onRetry,
}: StartupNoticeProps) {
  return (
    <div className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-control border border-warn/30 bg-panel px-3 py-2 text-warn">
      <AlertTriangle className="h-4 w-4 shrink-0 text-warn" aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">Inicio en modo limitado</p>
        <p className="truncate text-caption text-warn">{errors.join(' · ')}</p>
      </div>
      <Button
        variant="secondary"
        size="xs"
        disabled={retrying}
        onClick={onRetry}
      >
        <RefreshCw
          className={`h-3 w-3 ${retrying ? 'animate-pulse-dot' : ''}`}
          aria-hidden
        />
        {retrying ? 'Reintentando' : 'Reintentar'}
      </Button>
    </div>
  )
}
