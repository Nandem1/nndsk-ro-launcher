# Design baseline

Historical evidence for the centralization at `e09b9a7`, before the visual rework.
The current theme intentionally changes these values; see [design-system.md](design-system.md).

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
It also checks original rule order so equal sets cannot hide a cascade regression.

After centralization: zero raw palette/overlay classes, zero arbitrary pixel text
sizes, zero legacy radius/shadow classes. The 91 radius references, 27 shadow
references and 143 arbitrary text references now use named tokens; repeated input
and modal styles live in shared primitives. CSS preserves all 505 original rules
and the same 121 color, 6 radius, 13 shadow and 14 font-size resolved values.

Chromium verification against the original commit uses isolated fixture IPC (no
game launches, local configuration or real server data). Preparation (396 elements),
server modal (466), in-game (244), scanner modal (254) and active tools (246) have
identical computed styles, geometry and screenshot pixels in the production Vite
bundle, with no console errors. The development build was checked as well.
Animations are frozen to compare the same frame. PNGs and DOM snapshots are in
`/tmp/ro-design-{baseline,production}-{prep,server,ingame,scanner,active}.{png,json}`.
This verifies the frontend; real Tauri/WebKit and game integration are separate
desktop checks, not inferred from fixture results.
