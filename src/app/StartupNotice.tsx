import { Circle, RefreshCw } from 'lucide-react'
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
    <div className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-control bg-warn/[0.09] px-3 py-2">
      <Circle
        className="h-2 w-2 shrink-0 text-warn"
        strokeWidth={0}
        fill="currentColor"
        aria-hidden
      />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold text-muted">
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
