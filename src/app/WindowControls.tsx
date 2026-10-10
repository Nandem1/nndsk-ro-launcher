import { Minus, X } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { IconButton } from '../shared/ui/Button'

export function WindowControls() {
  return (
    <div className="flex items-center" data-tauri-drag-region="false">
      <IconButton
        label="Minimizar"
        size="sm"
        onClick={() => {
          void getCurrentWindow().minimize()
        }}
      >
        <Minus className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton
        label="Cerrar"
        size="sm"
        className="hover:text-bad"
        onClick={() => {
          void getCurrentWindow().close()
        }}
      >
        <X className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
    </div>
  )
}
