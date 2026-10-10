const PROBE_WARN = /\[AutoPot\].*Probe falló/i

export function isLogError(line: string): boolean {
  if (PROBE_WARN.test(line)) return false
  return (
    /\berr:/i.test(line) ||
    /^ERROR/i.test(line) ||
    /ERROR|falló|FAIL/i.test(line)
  )
}

export function countLogErrors(logs: string[]): number {
  return logs.filter(isLogError).length
}

export function logLineClass(line: string): string {
  if (isLogError(line)) return 'text-bad'
  if (/\bwarn:/i.test(line) || PROBE_WARN.test(line)) {
    return 'text-warn'
  }
  if (
    /Juego cerrado|Lanzando|Configurando|\[AutoPot\] Probe OK|\[Launch\]/i.test(
      line,
    )
  ) {
    return 'text-ok'
  }
  if (/\[AutoPot\]/i.test(line)) {
    return 'text-info'
  }
  return 'text-muted'
}
