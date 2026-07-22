import { create } from 'zustand'
import type { LaunchStatus } from '../features/launcher/launcher.store'

export type UiMode = 'prep' | 'ingame'

/** Vista de herramientas seleccionada manualmente (independiente de `mode`). */
export type ToolView = 'combat' | 'buffs'

interface UiModeState {
  mode: UiMode
  railPeek: boolean
  toolView: ToolView
  setMode: (mode: UiMode) => void
  toggleRailPeek: () => void
  setToolView: (view: ToolView) => void
}

export const useUiModeStore = create<UiModeState>((set) => ({
  mode: 'prep',
  railPeek: false,
  toolView: 'combat',
  setMode: (mode) => set({ mode, railPeek: false }),
  toggleRailPeek: () => set((s) => ({ railPeek: !s.railPeek })),
  setToolView: (toolView) => set({ toolView }),
}))

/** `ingame` desde `launching` para dar feedback inmediato al click en Jugar. */
export function modeForStatus(status: LaunchStatus, activeClients = 0): UiMode {
  return status === 'launching' || activeClients > 0 ? 'ingame' : 'prep'
}
