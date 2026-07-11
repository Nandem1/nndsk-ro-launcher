import { create } from 'zustand'
import { DEFAULT_AUTOBUFF_CONFIG } from '../../shared/constants'
import type { AutobuffStatusEvent } from '../../shared/types'

interface AutobuffState { status: AutobuffStatusEvent; busy: boolean; userEnabled: boolean; setStatus: (status: AutobuffStatusEvent) => void; setBusy: (busy: boolean) => void; setUserEnabled: (enabled: boolean) => void; reset: () => void }
const idle = (): AutobuffStatusEvent => ({ active: false, activeStatuses: 0, lastAppliedRule: null, delayMs: DEFAULT_AUTOBUFF_CONFIG.delayMs, error: null })
export const useAutobuffStore = create<AutobuffState>((set) => ({ status: idle(), busy: false, userEnabled: false, setStatus: (status) => set({ status }), setBusy: (busy) => set({ busy }), setUserEnabled: (userEnabled) => set({ userEnabled }), reset: () => set({ status: idle(), busy: false, userEnabled: false }) }))
