import type { ComponentPropsWithoutRef } from 'react'

interface ModalShellProps extends ComponentPropsWithoutRef<'div'> {
  layer?: 'server' | 'launch' | 'scanner'
}

const LAYER_CLASSES = {
  server: 'z-50 bg-overlay-dark/60',
  launch: 'z-[60] bg-overlay-dark/70',
  scanner: 'z-[70] bg-overlay-dark/70',
}

export function ModalShell({
  layer = 'server',
  className = '',
  children,
  ...rest
}: ModalShellProps) {
  return (
    <div
      className={`fixed inset-0 flex items-center justify-center p-4 backdrop-blur-panel ${LAYER_CLASSES[layer]} ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

export function modalSurfaceClasses(variant: 'plain' | 'glass' = 'plain') {
  return variant === 'glass'
    ? 'border border-overlay-light/[0.08] bg-panel-gradient from-panel-raised/95 to-panel/95 rounded-modal flex flex-col shadow-panel shadow-modal animate-scale-in overflow-hidden'
    : 'rounded-modal border border-overlay-light/[0.08] bg-panel p-5 shadow-modal'
}
