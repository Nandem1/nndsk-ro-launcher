type DotStatus = 'ok' | 'warning' | 'error' | 'neutral'

const dotClasses: Record<DotStatus, string> = {
  ok: 'bg-ok shadow-dot-ok',
  warning: 'bg-accent shadow-dot-warn',
  error: 'bg-bad shadow-dot-bad',
  neutral: 'bg-line-strong',
}

export function StatusDot({
  status,
  pulse = false,
}: {
  status: DotStatus
  pulse?: boolean
}) {
  return (
    <span
      className={`inline-block w-2 h-2 rounded-pill shrink-0 ${dotClasses[status]} ${
        pulse ? 'animate-pulse-dot' : ''
      }`}
      aria-hidden
    />
  )
}

export type { DotStatus }
