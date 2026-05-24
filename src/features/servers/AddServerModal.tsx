import { open } from '@tauri-apps/plugin-dialog'
import { useState } from 'react'
import { useServersStore } from './servers.store'

interface Props {
  onClose: () => void
}

function nameFromExePath(path: string): string {
  const file = path.split(/[/\\]/).pop() ?? ''
  return file.replace(/\.exe$/i, '')
}

export function AddServerModal({ onClose }: Props) {
  const addServer = useServersStore((s) => s.addServer)
  const [name, setName] = useState('')
  const [exePath, setExePath] = useState('')
  const [saving, setSaving] = useState(false)
  const [picking, setPicking] = useState(false)
  const [pickError, setPickError] = useState<string | null>(null)

  const handlePickExe = async () => {
    setPicking(true)
    setPickError(null)
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: 'Seleccionar ejecutable del cliente',
        filters: [{ name: 'Ejecutable', extensions: ['exe'] }],
      })

      if (!selected || Array.isArray(selected)) return

      setExePath(selected)
      if (!name.trim()) {
        setName(nameFromExePath(selected))
      }
    } catch (err) {
      setPickError(String(err))
    } finally {
      setPicking(false)
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim() || !exePath.trim()) return

    setSaving(true)
    try {
      const id = `server-${Date.now().toString(36)}`
      await addServer({
        id,
        name: name.trim(),
        executablePath: exePath.trim(),
      })
      onClose()
    } finally {
      setSaving(false)
    }
  }

  const selectedFile = exePath ? (exePath.split(/[/\\]/).pop() ?? exePath) : null

  return (
    <div
      className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="bg-zinc-900 border border-zinc-700/80 rounded-2xl p-6 w-[420px] flex flex-col gap-4 shadow-2xl shadow-black/40">
        <div>
          <h3 className="text-zinc-100 font-semibold text-lg">Agregar servidor</h3>
          <p className="text-xs text-zinc-500 mt-1">Selecciona el .exe del cliente de Ragnarok Online</p>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] text-zinc-500 uppercase tracking-wider">Ejecutable</label>
            <button
              type="button"
              onClick={handlePickExe}
              disabled={picking}
              className="w-full flex items-center justify-between gap-3 bg-zinc-950/60 border border-zinc-700/80
                rounded-lg px-3 py-2.5 text-sm text-left hover:border-amber-500/40 transition-colors
                disabled:opacity-50 disabled:cursor-wait"
            >
              <span className={selectedFile ? 'text-zinc-100 truncate' : 'text-zinc-600'}>
                {picking ? 'Abriendo...' : selectedFile ?? 'Seleccionar .exe...'}
              </span>
              <span className="text-xs text-amber-400 shrink-0">Examinar</span>
            </button>
            {exePath && (
              <p className="text-[10px] text-zinc-600 font-mono truncate px-1" title={exePath}>
                {exePath}
              </p>
            )}
            {pickError && <p className="text-xs text-red-400">{pickError}</p>}
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] text-zinc-500 uppercase tracking-wider">Nombre</label>
            <input
              autoFocus
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Mi Servidor RO"
              className="bg-zinc-950/60 border border-zinc-700/80 rounded-lg px-3 py-2.5 text-sm text-zinc-100
                placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20"
            />
          </div>

          <div className="flex gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 py-2.5 rounded-xl text-sm text-zinc-400 hover:text-zinc-100 border border-zinc-700/80 hover:border-zinc-500 transition-colors"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={!name.trim() || !exePath.trim() || saving}
              className="flex-1 py-2.5 rounded-xl text-sm font-semibold bg-amber-500 hover:bg-amber-400 text-zinc-950
                disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {saving ? 'Guardando...' : 'Agregar'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
