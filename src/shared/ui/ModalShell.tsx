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
      className={`fixed inset-0 flex items-center justify-center p-4 animate-modal-fade ${LAYER_CLASSES[layer]} ${className}`}
      {...rest}
    >
      {children}
    </div>
  )
}

export function modalSurfaceClasses(variant: 'plain' | 'glass' = 'plain') {
  return variant === 'glass'
    ? 'border border-line-strong bg-modal rounded-modal flex flex-col overflow-hidden'
    : 'rounded-modal border border-line-strong bg-modal p-5'
}
