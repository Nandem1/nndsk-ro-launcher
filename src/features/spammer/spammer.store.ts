import { create } from 'zustand'
import { DEFAULT_SPAMMER_CONFIG } from '../../shared/constants'
import type { SpammerStatusEvent } from '../../shared/types'

interface SpammerStore {
  status: SpammerStatusEvent
  busy: boolean
  userEnabled: boolean
  setStatus: (status: SpammerStatusEvent) => void
  setBusy: (busy: boolean) => void
  setUserEnabled: (enabled: boolean) => void
  reset: () => void
}

const idleStatus = (): SpammerStatusEvent => ({
  active: false,
  effectiveDelayMs: DEFAULT_SPAMMER_CONFIG.delayMs,
  armed: false,
  spamming: false,
  key: '',
  delayMs: DEFAULT_SPAMMER_CONFIG.delayMs,
  cycleCount: 0,
  error: null,
  gearMode: null,
})

export const useSpammerStore = create<SpammerStore>((set) => ({
  status: idleStatus(),
  busy: false,
  userEnabled: false,
  setStatus: (status) =>
    set((s) => (statusEquals(s.status, status) ? s : { status })),
  setBusy: (busy) => set((s) => (s.busy === busy ? s : { busy })),
  setUserEnabled: (userEnabled) =>
    set((s) => (s.userEnabled === userEnabled ? s : { userEnabled })),
  reset: () =>
    set({
      status: idleStatus(),
      busy: false,
      userEnabled: false,
    }),
}))

function statusEquals(a: SpammerStatusEvent, b: SpammerStatusEvent): boolean {
  return (
    a.active === b.active &&
    a.effectiveDelayMs === b.effectiveDelayMs &&
    a.armed === b.armed &&
    a.spamming === b.spamming &&
    a.key === b.key &&
    a.delayMs === b.delayMs &&
    a.cycleCount === b.cycleCount &&
    a.error === b.error &&
    a.gearMode === b.gearMode
  )
}
