import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { Panel } from '../../shared/ui/Panel'

const PROBE_WARN = /\[AutoPot\].*Probe falló/i

function isError(line: string): boolean {
  if (PROBE_WARN.test(line)) return false
  return /\berr:/i.test(line) || /^ERROR/i.test(line) || /ERROR|falló|FAIL/i.test(line)
}

function lineClass(line: string): string {
  if (isError(line)) return 'text-red-400'
  if (/\bwarn:/i.test(line) || PROBE_WARN.test(line)) {
    return 'text-amber-400'
  }
  if (/Juego cerrado|Lanzando|Configurando|\[AutoPot\] Probe OK|\[Launch\]/i.test(line)) {
    return 'text-emerald-400/80'
  }
  if (/\[AutoPot\]/i.test(line)) {
    return 'text-sky-400/90'
  }
  return 'text-zinc-400'
}

interface LogPanelViewProps {
  title: string
  logs: string[]
  emptyLabel?: string
  onClear: () => void
  className?: string
  leading?: ReactNode
}

export function LogPanelView({
  title,
  logs,
  emptyLabel = 'Sin actividad...',
  onClear,
  className = 'flex-1 min-h-0',
  leading,
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

  const errorLines = useMemo(() => logs.filter(isError), [logs])

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
      action={
        logs.length > 0 ? (
          <div className="flex gap-2">
            {errorLines.length > 0 && (
              <button
                onClick={copyErrors}
                className="text-[10px] text-red-400/80 hover:text-red-300 transition-colors uppercase tracking-wider"
              >
                {copiedErrors ? '¡Copiado!' : `Errores (${errorLines.length})`}
              </button>
            )}
            <button
              onClick={copyAll}
              className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors uppercase tracking-wider"
            >
              {copiedAll ? '¡Copiado!' : 'Copiar'}
            </button>
            <button
              onClick={onClear}
              className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors uppercase tracking-wider"
            >
              Limpiar
            </button>
          </div>
        ) : undefined
      }
    >
      <div className="flex-1 min-h-0 bg-zinc-950/50 rounded-lg border border-zinc-800/60 overflow-y-auto font-mono text-[11px] leading-relaxed px-3 py-2">
        {logs.length === 0 ? (
          <p className="text-zinc-600 select-none">{emptyLabel}</p>
        ) : (
          logs.map((line, i) => (
            <div key={i} className={`break-all ${lineClass(line)}`}>
              {line}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </Panel>
  )
}
