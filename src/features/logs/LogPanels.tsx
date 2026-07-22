import { useMemo, useState } from 'react'
import { useLauncherStore } from '../launcher/launcher.store'
import { countLogErrors } from './logs.logic'
import { useLogsStore } from './logs.store'
import { LogPanelView } from './LogPanelView'

type LogChannel = 'game' | 'tools'

function LogTab({
  active,
  onClick,
  children,
  badge,
}: {
  active: boolean
  onClick: () => void
  children: string
  badge?: number
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`px-2 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider transition-colors inline-flex items-center gap-1 ${
        active
          ? 'bg-amber-500/15 text-amber-300 border border-amber-500/25'
          : 'text-zinc-500 hover:text-zinc-300 border border-transparent'
      }`}
    >
      {children}
      {badge != null && badge > 0 && (
        <span className="px-1 min-w-[14px] text-center rounded bg-red-500/20 text-red-400 text-[9px] leading-tight">
          {badge}
        </span>
      )}
    </button>
  )
}

export function UnifiedLogPanel() {
  const isRunning = useLauncherStore((s) =>
    s.clients.some((client) => client.status === 'running'),
  )
  const [channel, setChannel] = useState<LogChannel>('game')
  const gameLogs = useLogsStore((s) => s.gameLogs)
  const toolLogs = useLogsStore((s) => s.toolLogs)
  const clearGameLogs = useLogsStore((s) => s.clearGameLogs)
  const clearToolLogs = useLogsStore((s) => s.clearToolLogs)

  const toolErrorCount = useMemo(() => countLogErrors(toolLogs), [toolLogs])

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
      compact
      leading={
        <div className="flex gap-1">
          <LogTab
            active={channel === 'game'}
            onClick={() => setChannel('game')}
          >
            Juego
          </LogTab>
          <LogTab
            active={channel === 'tools'}
            onClick={() => setChannel('tools')}
            badge={toolErrorCount}
          >
            Tools
          </LogTab>
          {isRunning && channel === 'game' && toolErrorCount > 0 && (
            <span className="text-[9px] text-zinc-600 self-center ml-0.5">
              · {toolErrorCount} en Tools
            </span>
          )}
        </div>
      }
    />
  )
}
