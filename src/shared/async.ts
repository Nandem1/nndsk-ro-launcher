import { toErrorMessage } from './errors'

export type AsyncResult<T> =
  { ok: true; value: T } | { ok: false; error: string }

/** Ejecuta una promesa y devuelve éxito/error sin lanzar. */
export async function runSafely<T>(
  fn: () => Promise<T>,
): Promise<AsyncResult<T>> {
  try {
    const value = await fn()
    return { ok: true, value }
  } catch (err) {
    return { ok: false, error: toErrorMessage(err) }
  }
}
