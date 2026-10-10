import { Circle, X } from 'lucide-react'

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
      className="mx-3 mt-3 flex shrink-0 items-center gap-3 rounded-control bg-warn/[0.09] px-3 py-2"
      role="status"
    >
      <Circle
        className="h-2 w-2 shrink-0 text-warn"
        strokeWidth={0}
        fill="currentColor"
        aria-hidden
      />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold text-muted">
          Mantenimiento completado
        </p>
        <p className="text-caption text-muted break-words">
          {notices.map((notice) => notice.message).join(' · ')}
        </p>
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Descartar aviso"
        className="flex h-6 w-6 items-center justify-center rounded-inline text-muted transition-colors hover:bg-panel-raised hover:text-ink"
      >
        <X className="h-3.5 w-3.5" aria-hidden />
      </button>
    </div>
  )
}
