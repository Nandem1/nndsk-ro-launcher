import { useState } from 'react'
import { useLogsStore } from './logs.store'
import { LogPanelView } from './LogPanelView'

type LogChannel = 'game' | 'tools'

function LogTab({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`px-2 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider transition-colors ${
        active
          ? 'bg-amber-500/15 text-amber-300 border border-amber-500/25'
          : 'text-zinc-500 hover:text-zinc-300 border border-transparent'
      }`}
    >
      {children}
    </button>
  )
}

export function UnifiedLogPanel() {
  const [channel, setChannel] = useState<LogChannel>('game')
  const gameLogs = useLogsStore((s) => s.gameLogs)
  const toolLogs = useLogsStore((s) => s.toolLogs)
  const clearGameLogs = useLogsStore((s) => s.clearGameLogs)
  const clearToolLogs = useLogsStore((s) => s.clearToolLogs)

  const logs = channel === 'game' ? gameLogs : toolLogs
  const onClear = channel === 'game' ? clearGameLogs : clearToolLogs
  const emptyLabel =
    channel === 'game'
      ? 'Wine / setup / lanzamiento...'
      : 'AutoPot / PID / memoria...'

  return (
    <LogPanelView
      title="Logs"
      logs={logs}
      emptyLabel={emptyLabel}
      onClear={onClear}
      className="flex-1 min-h-0"
      leading={
        <div className="flex gap-1">
          <LogTab active={channel === 'game'} onClick={() => setChannel('game')}>
            Juego
          </LogTab>
          <LogTab active={channel === 'tools'} onClick={() => setChannel('tools')}>
            Tools
          </LogTab>
        </div>
      }
    />
  )
}
