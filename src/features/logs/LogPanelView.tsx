import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { Panel } from '../../shared/ui/Panel'
import { isLogError, logLineClass } from './logs.logic'

interface LogPanelViewProps {
  title: string
  logs: string[]
  emptyLabel?: string
  onClear: () => void
  className?: string
  leading?: ReactNode
  compact?: boolean
}

export function LogPanelView({
  title,
  logs,
  emptyLabel = 'Sin actividad...',
  onClear,
  className = 'flex-1 min-h-0',
  leading,
  compact = false,
}: LogPanelViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>()
  const [copiedAll, setCopiedAll] = useState(false)
  const [copiedErrors, setCopiedErrors] = useState(false)

  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current)
    }
  }, [])

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'auto' })
  }, [logs])

  const errorLines = useMemo(() => logs.filter(isLogError), [logs])

  function copyWithFeedback(text: string, setFlag: (value: boolean) => void) {
    navigator.clipboard.writeText(text)
    setFlag(true)
    if (copyTimerRef.current) clearTimeout(copyTimerRef.current)
    copyTimerRef.current = setTimeout(() => setFlag(false), 1500)
  }

  function copyAll() {
    copyWithFeedback(logs.join('\n'), setCopiedAll)
  }

  function copyErrors() {
    copyWithFeedback(errorLines.join('\n'), setCopiedErrors)
  }

  return (
    <Panel
      title={title}
      className={className}
      leading={leading}
      compact={compact}
      action={
        logs.length > 0 ? (
          <div className="flex gap-2">
            {errorLines.length > 0 && (
              <button
                onClick={copyErrors}
                className="text-caption text-bad-bright/80 hover:text-bad-soft transition-colors uppercase tracking-wider"
              >
                {copiedErrors ? '¡Copiado!' : `Errores (${errorLines.length})`}
              </button>
            )}
            <button
              onClick={copyAll}
              className="text-caption text-muted hover:text-ink-dim transition-colors uppercase tracking-wider"
            >
              {copiedAll ? '¡Copiado!' : 'Copiar'}
            </button>
            <button
              onClick={onClear}
              className="text-caption text-muted hover:text-ink-dim transition-colors uppercase tracking-wider"
            >
              Limpiar
            </button>
          </div>
        ) : undefined
      }
    >
      <div className="flex-1 min-h-0 bg-surface/50 rounded-control border border-overlay-light/[0.04] overflow-y-auto font-mono text-detail leading-relaxed px-3 py-2">
        {logs.length === 0 ? (
          <p className="text-line-strong select-none">{emptyLabel}</p>
        ) : (
          logs.map((line, i) => (
            <div key={i} className={`break-all ${logLineClass(line)}`}>
              {line}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </Panel>
  )
}
