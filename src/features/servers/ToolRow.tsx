import { StatusDot, type DotStatus } from '../../shared/ui/StatusDot'
import { buttonClasses } from '../../shared/ui/Button'

interface ToolRowProps {
  label: string
  dotStatus: DotStatus
  detail?: string | null
  warning?: string | null
  onAction?: () => void
  actionLabel?: string
  actionBusy?: boolean
  actionDisabled?: boolean
  onSecondary?: () => void
  secondaryLabel?: string
  secondaryBusy?: boolean
  secondaryDanger?: boolean
}

export function ToolRow({
  label,
  dotStatus,
  detail,
  warning,
  onAction,
  actionLabel,
  actionBusy,
  actionDisabled,
  onSecondary,
  secondaryLabel,
  secondaryBusy,
  secondaryDanger,
}: ToolRowProps) {
  const actionClass = `${buttonClasses('secondary', 'xs')} shrink-0 disabled:cursor-not-allowed`

  const secondaryClass = secondaryDanger
    ? `${buttonClasses('danger', 'xs')} shrink-0 disabled:cursor-not-allowed`
    : actionClass

  return (
    <div className="flex flex-col gap-1 py-2.5 border-t border-line first:border-0">
      <div className="flex items-center gap-2.5 min-w-0">
        <StatusDot status={dotStatus} />
        <span className="text-sm text-ink shrink-0 w-20">{label}</span>
        {detail && (
          <span
            className="text-xs text-muted break-words min-w-0 flex-1 font-mono"
            title={detail}
          >
            {detail}
          </span>
        )}
        <div className="flex items-center gap-1.5 shrink-0">
          {onSecondary && secondaryLabel && (
            <button
              type="button"
              onClick={onSecondary}
              disabled={secondaryBusy}
              className={secondaryClass}
            >
              {secondaryBusy ? `${secondaryLabel}...` : secondaryLabel}
            </button>
          )}
          {onAction && actionLabel && (
            <button
              type="button"
              onClick={onAction}
              disabled={actionDisabled || actionBusy}
              className={actionClass}
            >
              {actionBusy ? `${actionLabel}...` : actionLabel}
            </button>
          )}
        </div>
      </div>
      {warning && (
        <p className="text-xs text-muted pl-[18px] leading-relaxed">
          {warning}
        </p>
      )}
    </div>
  )
}
