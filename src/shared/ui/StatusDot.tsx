import type { Tone } from './types'

type DotStatus = 'ok' | 'warning' | 'error' | 'neutral'

const dotClasses: Record<Tone, string> = {
  ok: 'bg-ok shadow-dot-ok',
  warn: 'bg-accent shadow-dot-warn',
  bad: 'bg-bad shadow-dot-bad',
  info: 'bg-info/15',
  neutral: 'bg-line-strong',
}

export function StatusDot({
  status,
  tone,
  pulse = false,
}: {
  status?: DotStatus
  tone?: Tone
  pulse?: boolean
}) {
  const effectiveTone =
    tone ??
    (status === 'warning'
      ? 'warn'
      : status === 'error'
        ? 'bad'
        : (status ?? 'neutral'))
  return (
    <span
      className={`inline-block w-2 h-2 rounded-pill shrink-0 ${dotClasses[effectiveTone]} ${
        pulse ? 'animate-pulse-dot' : ''
      }`}
      aria-hidden
    />
  )
}

export type { DotStatus }
