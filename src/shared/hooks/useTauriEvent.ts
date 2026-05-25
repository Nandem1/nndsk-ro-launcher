import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useEffect, type DependencyList } from 'react'

/** Suscribe un listener Tauri con cleanup automático al desmontar. */
export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void,
  deps: DependencyList = [],
) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    let cancelled = false

    listen<T>(event, (e) => handler(e.payload)).then((fn) => {
      if (cancelled) {
        fn()
        return
      }
      unlisten = fn
    })

    return () => {
      cancelled = true
      unlisten?.()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- deps controladas por el caller
  }, deps)
}
