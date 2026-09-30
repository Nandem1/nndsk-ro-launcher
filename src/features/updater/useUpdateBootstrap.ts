import { useEffect, useRef } from 'react'
import { LAUNCHER_EVENTS } from '../../shared/constants'
import { useTauriEvent } from '../../shared/hooks/useTauriEvent'
import type { UpdateSnapshot } from '../../shared/types'
import { useUpdaterStore } from './updater.store'

/** Starts only after the main window path has completed (phase != loading). */
export function useUpdateBootstrap(enabled: boolean): void {
  const started = useRef(false)

  useEffect(() => {
    if (!enabled || started.current) return
    started.current = true
    void useUpdaterStore.getState().refresh()
    void useUpdaterStore.getState().check()
  }, [enabled])
}

export function useUpdateEvents(): void {
  const applySnapshot = useUpdaterStore((s) => s.applySnapshot)
  useTauriEvent<UpdateSnapshot>(LAUNCHER_EVENTS.UPDATE, applySnapshot)
}
