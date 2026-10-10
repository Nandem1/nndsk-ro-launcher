import { useId, useState, type ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import { IconButton } from './Button'

export function CollapsibleNotice({
  children,
  className = '',
}: {
  children: ReactNode
  className?: string
}) {
  const [expanded, setExpanded] = useState(false)
  const contentId = useId()

  return (
    <div className={`notice-warn flex items-start gap-2 ${className}`}>
      <div
        id={contentId}
        data-design-clamp={!expanded}
        className={`min-w-0 flex-1 break-words ${expanded ? '' : 'line-clamp-2'}`}
      >
        {children}
      </div>
      <IconButton
        label={expanded ? 'Mostrar menos' : 'Mostrar más'}
        size="xs"
        aria-expanded={expanded}
        aria-controls={contentId}
        onClick={() => setExpanded(!expanded)}
      >
        <ChevronDown
          className={`h-3.5 w-3.5 ${expanded ? 'rotate-180' : ''}`}
          aria-hidden
        />
      </IconButton>
    </div>
  )
}
