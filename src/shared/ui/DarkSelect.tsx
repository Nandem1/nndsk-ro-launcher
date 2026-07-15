import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown } from 'lucide-react'

interface Option {
  value: string
  label: string
}

interface Props {
  value: string
  options: Option[]
  onChange: (value: string) => void
  disabled?: boolean
  compact?: boolean
  keycap?: boolean
  placeholder?: string
}

interface MenuPosition {
  top: number
  left: number
  width: number
  maxHeight: number
  openUp: boolean
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
    openUp,
    top: openUp
      ? Math.max(VIEWPORT_PADDING_PX, rect.top - MENU_GAP_PX - maxHeight)
      : rect.bottom + MENU_GAP_PX,
  }
}

export function DarkSelect({
  value,
  options,
  onChange,
  disabled = false,
  compact = false,
  keycap = false,
  placeholder = 'Seleccionar...',
}: Props) {
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
      if (
        rootRef.current?.contains(target) ||
        menuRef.current?.contains(target)
      )
        return
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
            className={`z-[200] py-1 rounded-lg border border-white/[0.08] bg-zinc-950/90 backdrop-blur-sm shadow-glass overflow-y-auto overscroll-contain animate-scale-in ${
              menuPosition.openUp ? 'origin-bottom' : 'origin-top'
            }`}
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
                    className={`w-full text-left transition-colors truncate ${compact ? 'px-2 py-1.5 text-[11px]' : 'px-3 py-2 text-sm'}
                      ${
                        isSelected
                          ? 'bg-amber-600/25 text-amber-200'
                          : 'text-zinc-200 hover:bg-zinc-800/80 hover:text-zinc-100'
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
        className={`w-full flex items-center justify-between border text-left focus:outline-none focus:border-amber-500/60 focus:ring-1 focus:ring-amber-500/20
          transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed
          ${
            keycap
              ? 'border-amber-500/20 bg-amber-500/[0.04] font-medium text-amber-100/90 hover:border-amber-500/40 hover:bg-amber-500/[0.07] disabled:hover:border-amber-500/20'
              : 'border-zinc-700/80 bg-zinc-950 text-zinc-100 hover:border-zinc-600 disabled:hover:border-zinc-700/80'
          }
          ${compact ? 'gap-1 rounded-md px-2 py-1 text-[11px]' : 'gap-2 rounded-lg px-3 py-2 text-sm'}`}
      >
        <span className="truncate">{selected?.label ?? placeholder}</span>
        <ChevronDown
          className={`w-3.5 h-3.5 text-zinc-500 shrink-0 transition-transform duration-200 ease-out-quart ${open ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </button>
      {menu}
    </div>
  )
}
