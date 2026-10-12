import { useEffect } from 'react'
import { installationIsActive } from './installations.logic'
import { useInstallationsStore } from './installations.store'

export function useInstallationTracking() {
  const drafts = useInstallationsStore((state) => state.drafts)
  const load = useInstallationsStore((state) => state.load)
  const active = drafts.some((draft) => installationIsActive(draft.status))
  useEffect(() => {
    void load()
  }, [load])
  useEffect(() => {
    if (!active) return
    const timer = window.setInterval(() => void load(), 1_500)
    return () => window.clearInterval(timer)
  }, [active, load])
  return drafts.filter((draft) => draft.status !== 'completed')
}
