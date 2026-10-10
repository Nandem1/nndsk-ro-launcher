// CSS-only focus styling needs an explicit modality for text inputs: browsers
// also match :focus-visible for pointer-focused editable fields. Never prevent
// events, move focus, or persist this presentational marker.
export function installFocusModality() {
  const root = document.documentElement
  const keyboard = () => {
    root.dataset.focusModality = 'keyboard'
  }
  const pointer = () => {
    root.dataset.focusModality = 'pointer'
  }
  document.addEventListener('keydown', keyboard, true)
  document.addEventListener('pointerdown', pointer, true)
  return () => {
    document.removeEventListener('keydown', keyboard, true)
    document.removeEventListener('pointerdown', pointer, true)
    delete root.dataset.focusModality
  }
}
