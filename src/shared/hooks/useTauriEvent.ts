import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useEffect, useRef, useState } from 'react'

/** Suscribe un listener Tauri con cleanup automático al desmontar. */
export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler)
  const [ready, setReady] = useState(false)

  useEffect(() => {
    handlerRef.current = handler
  }, [handler])

  useEffect(() => {
    setReady(false)
    let unlisten: UnlistenFn | undefined
    let cancelled = false

    void listen<T>(event, (e) => {
      if (!cancelled) handlerRef.current(e.payload)
    })
      .then((fn) => {
        if (cancelled) {
          fn()
          return
        }
        unlisten = fn
        setReady(true)
      })
      .catch((error) => {
        console.error(`No se pudo escuchar el evento ${event}`, error)
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [event])
  return ready
}
