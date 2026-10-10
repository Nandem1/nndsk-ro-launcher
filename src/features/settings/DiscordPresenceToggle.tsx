import { Panel } from '../../shared/ui/Panel'
import { ToggleSwitch } from '../../shared/ui/ToggleSwitch'
import { useSettingsStore } from './settings.store'

export function DiscordPresenceToggle() {
  const enabled = useSettingsStore((state) => state.richPresenceEnabled)
  const saving = useSettingsStore((state) => state.savingPresence)
  const error = useSettingsStore((state) => state.error)
  const setEnabled = useSettingsStore((state) => state.setRichPresenceEnabled)

  return (
    <Panel
      title="Discord Rich Presence"
      size="compact"
      tone={error && saving ? 'warn' : 'neutral'}
      action={
        <ToggleSwitch
          checked={enabled}
          disabled={saving}
          onChange={(next) => void setEnabled(next)}
        />
      }
      className="shrink-0"
    >
      <p className="text-caption leading-relaxed text-muted">
        Publica servidor, personaje, nivel y mapa en tu perfil de Discord.
      </p>
      {saving && (
        <p className="mt-1 text-caption text-muted">Guardando selección...</p>
      )}
      {error && !saving && (
        <p role="alert" className="mt-1 text-caption text-bad">
          {error}
        </p>
      )}
    </Panel>
  )
}
