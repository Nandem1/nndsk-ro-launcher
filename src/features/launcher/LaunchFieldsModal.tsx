import { useEffect, useId, useState } from 'react'
import { SERVER_CONTRACT } from '../../shared/contracts'
import type { LaunchValues } from '../../shared/types'
import { Button } from '../../shared/ui/Button'
import { Input } from '../../shared/ui/Input'
import { ModalShell, modalSurfaceClasses } from '../../shared/ui/ModalShell'

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
    <ModalShell
      layer="launch"
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
        className={`max-h-[90vh] w-[390px] overflow-y-auto ${modalSurfaceClasses()}`}
      >
        <h3 id={titleId} className="text-base font-semibold text-ink">
          Iniciar {serverName}
        </h3>
        <p className="mt-1 text-xs leading-relaxed text-muted">
          Estos valores se usan sólo para este arranque. No se guardan en la
          configuración y el launcher los redacta de la salida que captura.
        </p>

        <div className="mt-4 flex flex-col gap-3">
          {fields.map((field, index) => (
            <label key={field} className="flex flex-col gap-1.5">
              <span className="text-detail font-sans text-muted">{field}</span>
              <Input
                className="font-mono tabular-nums"
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
              />
            </label>
          ))}
        </div>

        <div className="mt-3 flex items-start justify-between gap-3">
          <p className="text-caption leading-relaxed text-muted">
            Los valores se ocultan por defecto. El protocolo del cliente puede
            exponerlos temporalmente en los argumentos del proceso de Windows.
          </p>
          <button
            type="button"
            onClick={() => setShowValues((current) => !current)}
            className="shrink-0 text-caption text-muted hover:text-ink"
          >
            {showValues ? 'Ocultar' : 'Mostrar'}
          </button>
        </div>

        <div className="mt-5 flex gap-2">
          <Button
            variant="outline"
            size="dialog"
            type="button"
            onClick={() => {
              setValues({})
              onCancel()
            }}
          >
            Cancelar
          </Button>
          <Button
            variant="solid"
            size="dialog"
            type="submit"
            disabled={!complete}
          >
            Continuar
          </Button>
        </div>
      </form>
    </ModalShell>
  )
}
