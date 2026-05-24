import { useEffect } from 'react'
import { useSettingsStore } from './settings.store'
import { Panel } from '../../shared/ui/Panel'
import { DarkSelect } from '../../shared/ui/DarkSelect'

export function RunnerSelector() {
  const { runners, selectedRunner, loadSettings, loadRunners, setRunner } = useSettingsStore()

  useEffect(() => {
    loadSettings().then(loadRunners)
  }, [])

  if (runners.length === 0) return null

  return (
    <Panel title="Runner" className="shrink-0">
      <DarkSelect
        value={selectedRunner}
        options={runners.map((r) => ({ value: r.path, label: r.name }))}
        onChange={setRunner}
      />
    </Panel>
  )
}
