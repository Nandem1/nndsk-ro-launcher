export function segmentedClasses(compact = false) {
  return `shrink-0 flex gap-1 rounded-segmented border border-line-soft bg-panel p-[3px] ${compact ? 'h-7' : ''}`
}

export function segmentClasses(active: boolean, compact = false) {
  return `flex items-center justify-center gap-1.5 font-sans font-medium leading-[normal] transition-colors duration-120 ${compact ? 'h-5 rounded-control-compact px-2 text-xs' : 'flex-1 rounded-segment px-7 py-1.5 text-data'} ${active ? 'bg-line text-ink' : 'text-muted hover:bg-panel-raised hover:text-ink'}`
}
