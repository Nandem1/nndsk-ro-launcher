# Design baseline

Captured from `90a4980` before changing styles, on `refactor/design-architecture`.
Generated CSS and resolved snapshots are local evidence under `/tmp`, not repository artifacts.

| Source references (`src/**/*.{ts,tsx,css}`) | Before |
| --- | ---: |
| Palette colors | 531 |
| zinc / amber | 322 / 136 |
| White/black overlays | 33 |
| Border radii | 91 |
| Shadows (including arbitrary values) | 27 |
| Arbitrary text sizes | 143 |

Text sizes: 9px (10), 10px (97), 11px (35), 12px (1).
Radii: default (18), md (12), lg (31), xl (17), 2xl (3), full (10).
Most palette references: MemoryScannerModal (61), AdvancedSettings (48),
AutopotPanel (44), ServerConfigModal (41).

```sh
npm run build
npx tailwindcss -i src/index.css -o /tmp/ro-design-baseline.css
node scripts/design-css.mjs snapshot /tmp/ro-design-baseline.css /tmp/ro-design-baseline.json
```

The comparator resolves CSS variables and normalizes equivalent RGB/hex forms.
It compares color, radius, shadow and font-size value sets and every original rule's
declarations, including variant selectors and media/keyframe contexts. Renamed
utilities are supplied through a class map; changed values cannot be excused by it.
