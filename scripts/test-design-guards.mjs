import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'

for (const name of ['unknown', 'border', 'divider', 'valid']) {
  const result = spawnSync(
    process.execPath,
    ['scripts/check-design.mjs', `scripts/fixtures/design/${name}.tsx.fixture`],
    { encoding: 'utf8' },
  )
  assert.equal(
    result.status,
    name === 'valid' ? 0 : 1,
    `${name}: ${result.stdout}\n${result.stderr}`,
  )
  if (name === 'unknown')
    for (const prefix of [
      'bg',
      'text',
      'border',
      'ring',
      'rounded',
      'shadow',
      'fill',
      'stroke',
      'divide',
      'outline',
      'placeholder',
      'from',
      'to',
      'via',
    ])
      assert.match(result.stderr, new RegExp(`${prefix}-nonexistent`))
  if (name === 'border' || name === 'divider')
    assert.match(result.stderr, /without explicit border color/)
  process.stdout.write(`${name}: expected exit ${result.status}\n`)
}
