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
      className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-control border border-warn/30 bg-panel px-3 py-2 text-warn"
      role="status"
    >
      <CheckCircle2 className="h-4 w-4 shrink-0 text-warn" aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">Mantenimiento completado</p>
        <p className="truncate text-caption text-warn">
          {notices.map((notice) => notice.message).join(' · ')}
        </p>
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Descartar aviso"
        className="flex h-6 w-6 items-center justify-center rounded-inline text-warn transition-colors hover:bg-panel-raised hover:text-warn"
      >
        <X className="h-3.5 w-3.5" aria-hidden />
      </button>
    </div>
  )
}
