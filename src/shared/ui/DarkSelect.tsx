import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'

interface Option {
  value: string
  label: string
}

interface Props {
  value: string
  options: Option[]
  onChange: (value: string) => void
  disabled?: boolean
}

interface MenuPosition {
  top: number
  left: number
  width: number
  maxHeight: number
}

const MENU_GAP_PX = 4
const MENU_MAX_HEIGHT_PX = 192
const VIEWPORT_PADDING_PX = 8

function measureMenuPosition(trigger: HTMLElement): MenuPosition {
  const rect = trigger.getBoundingClientRect()
  const spaceBelow = window.innerHeight - rect.bottom - VIEWPORT_PADDING_PX
  const spaceAbove = rect.top - VIEWPORT_PADDING_PX
  const openUp = spaceBelow < MENU_MAX_HEIGHT_PX && spaceAbove > spaceBelow

  const maxHeight = Math.min(
    MENU_MAX_HEIGHT_PX,
    Math.max(96, openUp ? spaceAbove - MENU_GAP_PX : spaceBelow - MENU_GAP_PX),
  )

  return {
    left: rect.left,
    width: rect.width,
    maxHeight,
    top: openUp
      ? Math.max(VIEWPORT_PADDING_PX, rect.top - MENU_GAP_PX - maxHeight)
      : rect.bottom + MENU_GAP_PX,
  }
}

export function DarkSelect({ value, options, onChange, disabled = false }: Props) {
  const [open, setOpen] = useState(false)
  const [menuPosition, setMenuPosition] = useState<MenuPosition | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLUListElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)

  const selected = options.find((o) => o.value === value)

  const updateMenuPosition = useCallback(() => {
    if (!triggerRef.current) return
    setMenuPosition(measureMenuPosition(triggerRef.current))
  }, [])

  useLayoutEffect(() => {
    if (!open) {
      setMenuPosition(null)
      return
    }
    updateMenuPosition()
  }, [open, options.length, updateMenuPosition])

  useEffect(() => {
    if (!open) return

    function handleClickOutside(e: MouseEvent) {
      const target = e.target as Node
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) return
      setOpen(false)
    }

    function handleReposition() {
      updateMenuPosition()
    }

    document.addEventListener('mousedown', handleClickOutside)
    window.addEventListener('resize', handleReposition)
    window.addEventListener('scroll', handleReposition, true)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      window.removeEventListener('resize', handleReposition)
      window.removeEventListener('scroll', handleReposition, true)
    }
  }, [open, updateMenuPosition])

  const menu =
    open && menuPosition
      ? createPortal(
          <ul
            ref={menuRef}
            role="listbox"
            style={{
              position: 'fixed',
              top: menuPosition.top,
              left: menuPosition.left,
              width: menuPosition.width,
              maxHeight: menuPosition.maxHeight,
            }}
            className="z-[200] py-1 rounded-lg border border-zinc-700 bg-zinc-950 shadow-xl shadow-black/50 overflow-y-auto overscroll-contain"
          >
            {options.map((option) => {
              const isSelected = option.value === value
              return (
                <li key={option.value} role="option" aria-selected={isSelected}>
                  <button
                    type="button"
                    onClick={() => {
                      onChange(option.value)
                      setOpen(false)
                    }}
                    className={`w-full text-left px-3 py-2 text-sm transition-colors truncate
                      ${isSelected
                        ? 'bg-amber-600/25 text-amber-200'
                        : 'bg-zinc-950 text-zinc-200 hover:bg-zinc-800 hover:text-zinc-100'
                      }`}
                  >
                    {option.label}
                  </button>
                </li>
              )
            })}
          </ul>,
          document.body,
        )
      : null

  return (
    <div ref={rootRef} className="relative min-w-0">
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => !disabled && setOpen((v) => !v)}
        className="w-full flex items-center justify-between gap-2 bg-zinc-950 border border-zinc-700/80
          text-zinc-100 text-sm rounded-lg px-3 py-2 text-left
          hover:border-zinc-600 focus:outline-none focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20
          transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:border-zinc-700/80"
      >
        <span className="truncate">{selected?.label ?? 'Seleccionar...'}</span>
        <span
          className={`text-zinc-500 text-[10px] shrink-0 transition-transform ${open ? 'rotate-180' : ''}`}
          aria-hidden
        >
          ▼
        </span>
      </button>
      {menu}
    </div>
  )
}
