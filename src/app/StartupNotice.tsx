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
    <div className="mx-3 mt-3 flex shrink-0 items-center gap-3 border-t border-line bg-panel px-3 py-2">
      <AlertTriangle className="h-4 w-4 shrink-0 text-muted" aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold text-warn">
          Inicio en modo limitado
        </p>
        <p className="text-caption text-muted break-words">
          {errors.join(' · ')}
        </p>
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
