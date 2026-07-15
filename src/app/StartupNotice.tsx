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
    <div className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-amber-100">
      <AlertTriangle className="h-4 w-4 shrink-0 text-amber-400" aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">Inicio en modo limitado</p>
        <p className="truncate text-[10px] text-amber-200/70">
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
          className={`h-3 w-3 ${retrying ? 'animate-spin' : ''}`}
          aria-hidden
        />
        {retrying ? 'Reintentando' : 'Reintentar'}
      </Button>
    </div>
  )
}
