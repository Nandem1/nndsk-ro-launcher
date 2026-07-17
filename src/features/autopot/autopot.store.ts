import { create } from 'zustand'
import type { AutopotStatusEvent } from '../../shared/types'
import { DEFAULT_AUTOPOT_CONFIG } from '../../shared/constants'

interface AutopotState {
  status: AutopotStatusEvent
  busy: boolean
  userEnabled: boolean
  setStatus: (status: AutopotStatusEvent) => void
  setBusy: (busy: boolean) => void
  setUserEnabled: (enabled: boolean) => void
  reset: () => void
}

const idleStatus = (): AutopotStatusEvent => ({
  active: false,
  effectiveDelayMs: DEFAULT_AUTOPOT_CONFIG.delayMs,
  curHp: 0,
  maxHp: 0,
  curSp: 0,
  maxSp: 0,
  hpPercent: DEFAULT_AUTOPOT_CONFIG.hpPercent,
  spPercent: DEFAULT_AUTOPOT_CONFIG.spPercent,
  characterName: '',
  error: null,
})

export const useAutopotStore = create<AutopotState>((set) => ({
  status: idleStatus(),
  busy: false,
  userEnabled: false,
  setStatus: (status) =>
    set((s) => (statusEquals(s.status, status) ? s : { status })),
  setBusy: (busy) => set((s) => (s.busy === busy ? s : { busy })),
  setUserEnabled: (userEnabled) =>
    set((s) => (s.userEnabled === userEnabled ? s : { userEnabled })),
  reset: () => set({ status: idleStatus(), busy: false, userEnabled: false }),
}))

function statusEquals(a: AutopotStatusEvent, b: AutopotStatusEvent): boolean {
  return (
    a.active === b.active &&
    a.effectiveDelayMs === b.effectiveDelayMs &&
    a.curHp === b.curHp &&
    a.maxHp === b.maxHp &&
    a.curSp === b.curSp &&
    a.maxSp === b.maxSp &&
    a.hpPercent === b.hpPercent &&
    a.spPercent === b.spPercent &&
    a.characterName === b.characterName &&
    a.error === b.error
  )
}
