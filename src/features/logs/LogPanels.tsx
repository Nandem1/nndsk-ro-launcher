import { useMemo, useState } from 'react'
import { useLauncherStore } from '../launcher/launcher.store'
import { countLogErrors } from './logs.logic'
import { useLogsStore } from './logs.store'
import { LogPanelView } from './LogPanelView'
import {
  segmentedClasses,
  segmentClasses,
} from '../../shared/ui/segmentedControl'

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
      className={segmentClasses(active, true)}
    >
      {children}
      {badge != null && badge > 0 && (
        <span className="px-1 min-w-[14px] text-center text-bad text-micro leading-tight">
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
          <div data-design-log-tabs className={segmentedClasses(true)}>
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
          </div>
          {isRunning && channel === 'game' && toolErrorCount > 0 && (
            <span className="text-micro text-muted self-center ml-0.5">
              · {toolErrorCount} en Tools
            </span>
          )}
        </div>
      }
    />
  )
}
