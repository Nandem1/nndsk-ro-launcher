import { useEffect, useId, useState } from 'react'
import { SERVER_CONTRACT } from '../../shared/contracts'
import type { LaunchValues } from '../../shared/types'

interface Props {
  serverName: string
  fields: string[]
  onCancel: () => void
  onSubmit: (values: LaunchValues) => void
}

export function LaunchFieldsModal({
  serverName,
  fields,
  onCancel,
  onSubmit,
}: Props) {
  const [values, setValues] = useState<LaunchValues>({})
  const [showValues, setShowValues] = useState(false)
  const titleId = useId()

  const hasOwnValue = (field: string) =>
    Object.prototype.hasOwnProperty.call(values, field) &&
    typeof values[field] === 'string'
  const complete = fields.every(
    (field) => hasOwnValue(field) && values[field].length > 0,
  )

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setValues({})
        onCancel()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onCancel])

  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    if (!complete) return
    const submitted = { ...values }
    setValues({})
    onSubmit(submitted)
  }

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onCancel()
      }}
    >
      <form
        onSubmit={submit}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="max-h-[90vh] w-[390px] overflow-y-auto rounded-2xl border border-white/[0.08] bg-zinc-900 p-5 shadow-2xl"
      >
        <h3 id={titleId} className="text-base font-semibold text-zinc-100">
          Iniciar {serverName}
        </h3>
        <p className="mt-1 text-xs leading-relaxed text-zinc-500">
          Estos valores se usan sólo para este arranque. No se guardan en la
          configuración y el launcher los redacta de la salida que captura.
        </p>

        <div className="mt-4 flex flex-col gap-3">
          {fields.map((field, index) => (
            <label key={field} className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-zinc-500">
                {field}
              </span>
              <input
                autoFocus={index === 0}
                type={showValues ? 'text' : 'password'}
                autoComplete="off"
                maxLength={SERVER_CONTRACT.maxLaunchValueLength}
                value={hasOwnValue(field) ? values[field] : ''}
                onChange={(event) =>
                  setValues((current) => ({
                    ...current,
                    [field]: event.target.value,
                  }))
                }
                className="rounded-lg border border-zinc-700/80 bg-zinc-950/70 px-3 py-2.5 text-sm text-zinc-100 outline-none focus:border-amber-500/60"
              />
            </label>
          ))}
        </div>

        <div className="mt-3 flex items-start justify-between gap-3">
          <p className="text-[10px] leading-relaxed text-amber-400/80">
            Los valores se ocultan por defecto. El protocolo del cliente puede
            exponerlos temporalmente en los argumentos del proceso de Windows.
          </p>
          <button
            type="button"
            onClick={() => setShowValues((current) => !current)}
            className="shrink-0 text-[10px] text-zinc-500 hover:text-zinc-200"
          >
            {showValues ? 'Ocultar' : 'Mostrar'}
          </button>
        </div>

        <div className="mt-5 flex gap-2">
          <button
            type="button"
            onClick={() => {
              setValues({})
              onCancel()
            }}
            className="flex-1 rounded-xl border border-zinc-700 py-2.5 text-sm text-zinc-400 hover:text-zinc-100"
          >
            Cancelar
          </button>
          <button
            type="submit"
            disabled={!complete}
            className="flex-1 rounded-xl bg-amber-500 py-2.5 text-sm font-semibold text-zinc-950 disabled:opacity-40"
          >
            Continuar
          </button>
        </div>
      </form>
    </div>
  )
}
