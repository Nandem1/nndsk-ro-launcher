import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useEffect, useRef } from 'react'

/** Suscribe un listener Tauri con cleanup automático al desmontar. */
export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler)

  useEffect(() => {
    handlerRef.current = handler
  }, [handler])

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    let cancelled = false

    void listen<T>(event, (e) => handlerRef.current(e.payload))
      .then((fn) => {
        if (cancelled) {
          fn()
          return
        }
        unlisten = fn
      })
      .catch((error) => {
        console.error(`No se pudo escuchar el evento ${event}`, error)
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [event])
}
