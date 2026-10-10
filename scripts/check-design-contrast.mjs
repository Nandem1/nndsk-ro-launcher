import fs from 'node:fs'
import path from 'node:path'

const root = path.resolve(import.meta.dirname, '..')
const report = (message) => process.stdout.write(`${message}\n`)
const source = fs.readFileSync(path.join(root, 'src/index.css'), 'utf8')
const channels = Object.fromEntries(
  [...source.matchAll(/--c-([\w-]+):\s*([\d ]+);/g)].map(([, name, value]) => [
    name,
    value.trim().split(/\s+/).map(Number),
  ]),
)

// WCAG relative luminance, sRGB transfer function; no rounded intermediate values.
function luminance(rgb) {
  if (
    !rgb ||
    rgb.length !== 3 ||
    rgb.some((channel) => channel < 0 || channel > 255)
  ) {
    throw new Error('Missing or invalid RGB token')
  }
  return rgb
    .map((channel) => channel / 255)
    .map((channel) =>
      channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
    )
    .reduce(
      (sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index],
      0,
    )
}

const pairs = ['surface', 'panel', 'panel-raised'].flatMap((background) =>
  ['muted', 'ink', 'ok', 'bad', 'info', 'warn', 'accent'].map((foreground) => [
    foreground,
    background,
    4.5,
  ]),
)
pairs.push(['on-accent', 'accent', 4.5])
// Switch boundary against its surrounding surface, and knob against its track.
for (const surface of ['surface', 'panel', 'modal']) {
  pairs.push(['muted', surface, 3], ['ok', surface, 3])
}
pairs.push(['muted', 'field', 3], ['on-accent', 'ok', 3])
pairs.push(['muted', 'track', 3], ['surface', 'ok', 3], ['ink', 'track', 3])
// Idle/disabled text is not attenuated. Quiet field borders are decorative.
for (const surface of ['surface', 'panel', 'modal', 'field']) {
  pairs.push(['muted', surface, 4.5])
}
for (const surface of ['panel', 'panel-raised']) {
  pairs.push(['outline', surface, 3], ['accent', surface, 3])
}
let failed = false
for (const name of [
  'surface',
  'panel',
  'panel-raised',
  'field',
  'modal',
  'line',
  'line-strong',
]) {
  const spread = Math.max(...channels[name]) - Math.min(...channels[name])
  report(`${name} neutral spread: ${spread}`)
  if (spread > 6) failed = true
}
for (const [foreground, background, minimum] of pairs) {
  const a = luminance(channels[foreground])
  const b = luminance(channels[background])
  const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
  report(`${foreground} / ${background}: ${ratio.toFixed(3)}:1`)
  if (ratio < minimum) failed = true
}
if (failed) {
  console.error(
    'Design contrast/neutrality failed: neutral RGB spread needs ≤6; text needs 4.5:1; switch boundaries and knobs need 3:1',
  )
  process.exitCode = 1
}

const blend = (foreground, background, alpha) =>
  foreground.map(
    (channel, index) => channel * alpha + background[index] * (1 - alpha),
  )
const ratio = (foreground, background) =>
  (Math.max(luminance(foreground), luminance(background)) + 0.05) /
  (Math.min(luminance(foreground), luminance(background)) + 0.05)
for (const tone of ['bad', 'ok', 'info', 'warn']) {
  for (const alpha of [0.1, 0.15]) {
    const background = blend(channels[tone], channels.panel, alpha)
    const foreground =
      tone === 'bad' && alpha === 0.15 ? channels.ink : channels[tone]
    const contrast = ratio(foreground, background)
    report(`tonal ${tone}/${alpha}: ${contrast.toFixed(3)}:1`)
    if (contrast < 4.5) process.exitCode = 1
  }
}
const notice = blend(channels.warn, channels.panel, 0.09)
const noticeContrast = ratio(channels.muted, notice)
report(`notice muted/warn9% over panel: ${noticeContrast.toFixed(3)}:1`)
if (noticeContrast < 4.5) process.exitCode = 1

// Approved exceptions: line-soft, line and off-switch track are decorative.
for (const background of ['surface', 'panel', 'panel-raised']) {
  for (const foreground of ['line-soft', 'line', 'track'])
    report(
      `decorativo ${foreground}/${background}: ${ratio(channels[foreground], channels[background]).toFixed(3)}:1 (report only)`,
    )
  report(
    `focus accent/${background}: ${ratio(channels.accent, channels[background]).toFixed(3)}:1`,
  )
}
