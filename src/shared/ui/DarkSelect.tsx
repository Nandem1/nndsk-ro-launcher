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
  const [activeIndex, setActiveIndex] = useState(-1)
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
    if (!open || !menuPosition || activeIndex < 0) return
    const option =
      menuRef.current?.querySelectorAll<HTMLElement>('[role="option"]')[
        activeIndex
      ]
    option?.focus()
  }, [activeIndex, menuPosition, open])

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

  const openMenu = (preferredIndex?: number) => {
    if (disabled || options.length === 0) return
    const selectedIndex = options.findIndex((option) => option.value === value)
    setActiveIndex(preferredIndex ?? (selectedIndex >= 0 ? selectedIndex : 0))
    setOpen(true)
  }

  const chooseOption = (index: number) => {
    const option = options[index]
    if (!option) return
    onChange(option.value)
    setOpen(false)
    triggerRef.current?.focus()
  }

  const handleMenuKeyDown = (event: React.KeyboardEvent) => {
    if (!open) return
    if (
      ['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' ', 'Escape'].includes(
        event.key,
      )
    ) {
      event.preventDefault()
      event.stopPropagation()
    }
    if (event.key === 'Escape') {
      setOpen(false)
      triggerRef.current?.focus()
    } else if (event.key === 'ArrowDown') {
      setActiveIndex((index) => (index + 1) % options.length)
    } else if (event.key === 'ArrowUp') {
      setActiveIndex((index) => (index - 1 + options.length) % options.length)
    } else if (event.key === 'Home') {
      setActiveIndex(0)
    } else if (event.key === 'End') {
      setActiveIndex(options.length - 1)
    } else if (event.key === 'Enter' || event.key === ' ') {
      chooseOption(activeIndex)
    } else if (event.key === 'Tab') {
      setOpen(false)
    }
  }

  const menu =
    open && menuPosition
      ? createPortal(
          <ul
            ref={menuRef}
            role="listbox"
            onKeyDown={handleMenuKeyDown}
            style={{
              position: 'fixed',
              top: menuPosition.top,
              left: menuPosition.left,
              width: menuPosition.width,
              maxHeight: menuPosition.maxHeight,
            }}
            className={`z-[200] py-1 rounded-control border border-overlay-light/[0.08] bg-surface/90 backdrop-blur-panel shadow-panel overflow-y-auto overscroll-contain animate-scale-in ${
              menuPosition.openUp ? 'origin-bottom' : 'origin-top'
            }`}
          >
            {options.map((option, index) => {
              const isSelected = option.value === value
              return (
                <li key={option.value} role="presentation">
                  <button
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    tabIndex={index === activeIndex ? 0 : -1}
                    onMouseMove={() => setActiveIndex(index)}
                    onClick={() => chooseOption(index)}
                    className={`w-full text-left transition-colors truncate ${compact ? 'px-2 py-1.5 text-detail' : 'px-3 py-2 text-sm'}
                      ${
                        isSelected
                          ? 'bg-accent-strong/25 text-accent-soft'
                          : 'text-ink-bright hover:bg-panel-raised/80 hover:text-ink'
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
        onClick={() => {
          if (open) setOpen(false)
          else openMenu()
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            openMenu(event.key === 'ArrowUp' ? options.length - 1 : undefined)
          }
        }}
        className={`w-full flex items-center justify-between border text-left focus:outline-none focus:border-accent/60 focus:ring-1 focus:ring-accent/20
          transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed
          ${
            keycap
              ? 'border-accent/20 bg-accent/[0.04] font-medium text-accent-ink/90 hover:border-accent/40 hover:bg-accent/[0.07] disabled:hover:border-accent/20'
              : 'border-line/80 bg-surface text-ink hover:border-line-strong disabled:hover:border-line/80'
          }
          ${compact ? 'gap-1 rounded-control-compact px-2 py-1 text-detail' : 'gap-2 rounded-control px-3 py-2 text-sm'}`}
      >
        <span className="truncate">{selected?.label ?? placeholder}</span>
        <ChevronDown
          className={`w-3.5 h-3.5 text-muted shrink-0 transition-transform duration-200 ease-out-quart ${open ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </button>
      {menu}
    </div>
  )
}
