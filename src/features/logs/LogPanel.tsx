import { useEffect, useRef, useState } from 'react'
import { useLogsStore } from './logs.store'
import { Panel } from '../../shared/ui/Panel'

function lineClass(line: string): string {
  if (/\berr:/i.test(line) || /^ERROR/i.test(line)) return 'text-red-400'
  if (/\bwarn:/i.test(line)) return 'text-amber-400'
  if (/Juego cerrado|Lanzando|Configurando/i.test(line)) return 'text-emerald-400/80'
  return 'text-zinc-400'
}

function isError(line: string): boolean {
  return /\berr:/i.test(line) || /^ERROR/i.test(line)
}

export function LogPanel() {
  const logs = useLogsStore((s) => s.logs)
  const clearLogs = useLogsStore((s) => s.clearLogs)
  const bottomRef = useRef<HTMLDivElement>(null)
  const [copiedAll, setCopiedAll] = useState(false)
  const [copiedErrors, setCopiedErrors] = useState(false)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [logs])

  const errorLines = logs.filter(isError)

  function copyAll() {
    navigator.clipboard.writeText(logs.join('\n'))
    setCopiedAll(true)
    setTimeout(() => setCopiedAll(false), 1500)
  }

  function copyErrors() {
    navigator.clipboard.writeText(errorLines.join('\n'))
    setCopiedErrors(true)
    setTimeout(() => setCopiedErrors(false), 1500)
  }

  return (
    <Panel
      title="Logs"
      className="flex-1 min-h-0"
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
              onClick={clearLogs}
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
          <p className="text-zinc-600 select-none">Sin actividad...</p>
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
