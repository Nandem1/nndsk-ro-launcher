import { useCallback, useEffect, useRef, useState } from 'react'

const MINIMUM_DELAY_MS = 16
const MAXIMUM_DELAY_MS = 50

interface SpammerDelayControlProps {
  configuredDelayMs: number
  disabled: boolean
  onCommit: (delayMs: number) => void | Promise<void>
}

function clampDelay(delayMs: number) {
  return Math.min(MAXIMUM_DELAY_MS, Math.max(MINIMUM_DELAY_MS, delayMs))
}

export function SpammerDelayControl({
  configuredDelayMs,
  disabled,
  onCommit,
}: SpammerDelayControlProps) {
  const committedDelayMs = clampDelay(configuredDelayMs)
  const [draftDelayMs, setDraftDelayMs] = useState(committedDelayMs)
  const dirtyRef = useRef(false)

  useEffect(() => {
    if (!dirtyRef.current) setDraftDelayMs(committedDelayMs)
  }, [committedDelayMs])

  const commit = useCallback(
    (rawDelayMs: number) => {
      const delayMs = clampDelay(rawDelayMs)
      const changed = dirtyRef.current && delayMs !== committedDelayMs
      dirtyRef.current = false
      setDraftDelayMs(delayMs)
      if (changed) void onCommit(delayMs)
    },
    [committedDelayMs, onCommit],
  )

  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-zinc-600 uppercase tracking-wide shrink-0">
        Delay
      </span>
      <input
        type="range"
        aria-label="Delay del spammer"
        min={MINIMUM_DELAY_MS}
        max={MAXIMUM_DELAY_MS}
        step={1}
        disabled={disabled}
        value={draftDelayMs}
        onChange={(event) => {
          dirtyRef.current = true
          setDraftDelayMs(Number(event.target.value))
        }}
        onPointerUp={(event) => commit(Number(event.currentTarget.value))}
        onKeyUp={(event) => commit(Number(event.currentTarget.value))}
        onBlur={(event) => commit(Number(event.currentTarget.value))}
        className="flex-1 accent-amber-500 disabled:opacity-50"
      />
      <span className="text-[10px] text-zinc-500 w-8 text-right shrink-0">
        {draftDelayMs}ms
      </span>
    </div>
  )
}
