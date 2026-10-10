import fs from 'node:fs'
import path from 'node:path'

const root = path.resolve(import.meta.dirname, '..')
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

const pairs = ['surface', 'panel'].flatMap((background) =>
  ['muted', 'ok', 'bad', 'info', 'warn', 'accent'].map((foreground) => [
    foreground,
    background,
  ]),
)
pairs.push(['on-accent', 'accent'])
let failed = false
for (const [foreground, background] of pairs) {
  const a = luminance(channels[foreground])
  const b = luminance(channels[background])
  const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
  console.log(`${foreground} / ${background}: ${ratio.toFixed(3)}:1`)
  if (ratio < 4.5) failed = true
}
if (failed) {
  console.error('Design contrast failed: minimum 4.5:1')
  process.exitCode = 1
}
