import { useEffect, useRef, useState } from 'react'

interface Option {
  value: string
  label: string
}

interface Props {
  value: string
  options: Option[]
  onChange: (value: string) => void
}

export function DarkSelect({ value, options, onChange }: Props) {
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  const selected = options.find((o) => o.value === value)

  useEffect(() => {
    if (!open) return

    function handleClickOutside(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center justify-between gap-2 bg-zinc-950/60 border border-zinc-700/80
          text-zinc-100 text-sm rounded-lg px-3 py-2 text-left
          hover:border-zinc-600 focus:outline-none focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20
          transition-colors cursor-pointer"
      >
        <span className="truncate">{selected?.label ?? 'Seleccionar...'}</span>
        <span
          className={`text-zinc-500 text-[10px] shrink-0 transition-transform ${open ? 'rotate-180' : ''}`}
          aria-hidden
        >
          ▼
        </span>
      </button>

      {open && (
        <ul
          className="absolute z-50 left-0 right-0 mt-1 py-1 rounded-lg border border-zinc-700/80
            bg-zinc-900 shadow-xl shadow-black/40 max-h-48 overflow-y-auto"
        >
          {options.map((option) => {
            const isSelected = option.value === value
            return (
              <li key={option.value}>
                <button
                  type="button"
                  onClick={() => {
                    onChange(option.value)
                    setOpen(false)
                  }}
                  className={`w-full text-left px-3 py-2 text-sm transition-colors truncate
                    ${isSelected
                      ? 'bg-amber-500/15 text-amber-300'
                      : 'text-zinc-200 hover:bg-zinc-800 hover:text-zinc-100'
                    }`}
                >
                  {option.label}
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
