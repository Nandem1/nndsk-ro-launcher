import { CheckCircle2, X } from 'lucide-react'

import type { StorageNotice } from '../shared/types'

interface MaintenanceNoticeProps {
  notices: StorageNotice[]
  onDismiss: () => void
}

export function MaintenanceNotice({
  notices,
  onDismiss,
}: MaintenanceNoticeProps) {
  if (notices.length === 0) return null

  return (
    <div
      className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-control border border-accent/30 bg-accent/10 px-3 py-2 text-accent-ink"
      role="status"
    >
      <CheckCircle2
        className="h-4 w-4 shrink-0 text-accent-bright"
        aria-hidden
      />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">Mantenimiento completado</p>
        <p className="truncate text-caption text-accent-soft/70">
          {notices.map((notice) => notice.message).join(' · ')}
        </p>
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Descartar aviso"
        className="flex h-6 w-6 items-center justify-center rounded-inline text-accent-light/70 transition-colors hover:bg-accent/10 hover:text-accent-ink"
      >
        <X className="h-3.5 w-3.5" aria-hidden />
      </button>
    </div>
  )
}
