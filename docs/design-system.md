# Design system

Values live in `src/index.css`; `tailwind.config.js` exposes RGB channel variables
as `rgb(var(--c-name) / <alpha-value>)`. Opacity and state variants keep their
original values. The migration catalog in `scripts/design-token-map.mjs` contains
names only. See [the baseline](design-baseline.md) for measurements and comparison commands.

## Color tokens

| Token (`--c-*`) | Current | Rework objective (documentation only) |
| --- | --- | --- |
| surface | #09090B | #0F1412 |
| panel | #18181B | #161C19 |
| panel-raised | #27272A | Candidate for panel |
| line | #3F3F46 | #2A332E |
| line-strong | #52525B | #3A463F |
| muted | #71717A | #8A968D |
| ink-soft | #A1A1AA | Candidate for muted |
| ink-dim | #D4D4D8 | Candidate for ink |
| ink-bright | #E4E4E7 | Candidate for ink |
| ink | #F4F4F5 | #E8EEE8 |
| accent-ink | #FEF3C7 | Candidate for accent |
| accent-soft | #FDE68A | Candidate for accent |
| accent-light | #FCD34D | Candidate for accent |
| accent-bright | #FBBF24 | Candidate for accent |
| accent | #F59E0B | #FF5C39 |
| accent-strong | #D97706 | Candidate for accent |
| warn | Alias of accent | #E2A33A |
| ok-ink | #D1FAE5 | Candidate for ok |
| ok-soft | #6EE7B7 | Candidate for ok |
| ok-bright | #34D399 | Candidate for ok |
| ok | #10B981 | #7FB069 |
| bad-ink | #FEE2E2 | Candidate for bad |
| bad-soft | #FCA5A5 | Candidate for bad |
| bad-bright | #F87171 | Candidate for bad |
| bad | #EF4444 | #E0604A |
| bad-strong | #DC2626 | Candidate for bad |
| info-soft | #BAE6FD | Candidate for info |
| info-bright | #38BDF8 | Candidate for info |
| info | #0EA5E9 | #5B9BD5 |
| info-strong | #0284C7 | Candidate for info |
| special-ink | #DDD6FE | Undecided |
| special-soft | #C4B5FD | Undecided |
| special | #8B5CF6 | Undecided |
| overlay-light | #FFFFFF | Undecided |
| overlay-dark | #000000 | Undecided |

Merge candidates preserve distinct current values: panel/panel-raised,
muted/ink-soft, ink/ink-dim/ink-bright, the accent, ok, bad, info and special
families. `line` also appears in disabled text, and `line-strong` in neutral dots;
these roles should be reviewed during the rework. Accent and warning share one
value today; `warn` is an explicit alias so they can diverge later.

## Shape, text and effects

| Tokens | Current | Rework objective (documentation only) |
| --- | --- | --- |
| radius-inline / control-compact / control | 4 / 6 / 8px at 16px root | 0 |
| radius-panel / modal / pill | 12 / 16 / 9999px at 16px root | 0 |
| text-micro / caption / detail / label | 9 / 10 / 11 / 12px, inherited line height | Undecided |
| text-xs / sm / base / lg / xl | .75 / .875 / 1 / 1.125 / 1.25rem | Undecided |
| leading-xs / sm / base / lg / xl | 1 / 1.25 / 1.5 / 1.75 / 1.75rem | Undecided |
| font-sans / font-mono | Tailwind's original system stacks | IBM Plex Sans / IBM Plex Mono; Barlow Condensed for display |
| shadow-panel | inset 0 1px 0 0 white/5%, 0 8px 24px -12px black/70% | No glow |
| shadow-modal | 0 25px 50px -12px black/25% | Undecided |
| shadow-control | 0 1px 3px 0 black/10%, 0 1px 2px -1px black/10% | Undecided |
| shadow-glow-ok / warn / bad | 0 0 20px -6px corresponding color/30% | None |
| shadow-dot-ok / warn / bad | 0 0 6px corresponding color/50% | None |
| shadow-check-warn | inset 0 0 0 1px accent/6% | Undecided |
| blur-panel | 4px | None |
| gradient-panel-direction / progress-direction | To bottom / right, existing color stops | None |
| body-illumination | Original amber/8% and amber/4% radial gradients | None |

Current animations and timing remain defined in the theme and stylesheet without
changing duration, transforms, keyframes or reduced-motion behavior.

## Primitives

All color variants consume tokens and use `Tone = ok | warn | bad | info | neutral`.
Only existing visual strengths are exposed; the rework can update their maps in
`src/shared/ui/`. Features own content, dimensions and event handlers.

| Primitive | Variants / sizes / tones |
| --- | --- |
| Button / IconButton | primary, secondary, ghost; danger/success compatibility aliases; tone for primary; xs/sm/md/lg; `buttonClasses` has the same API |
| Button dialog actions | solid/outline; dialog/dialog-sm; tone for solid; preserves original native action behavior |
| Panel | glass/idle; default/compact/hero; semantic tones; legacy idle/success/warning/danger and compact/hero aliases retained |
| StatusDot | tone and pulse; legacy status=ok/warning/error/neutral retained; original fixed 8px dot |
| ToggleSwitch | tone (default ok); original fixed pill geometry and checked/disabled behavior |
| Checkbox | tone (default warn); original fixed 16px box and checked/disabled behavior |
| DarkSelect | default/keycap; sm/md; tone (default warn) for selected options; legacy compact/keycap aliases retained |
| ModalShell + modalSurfaceClasses | layers server/launch/scanner; surfaces plain/glass; caller retains dialog/form, widths, scroll and close logic |
| Input | modal/config; native input props/ref; caller retains spinner, disabled and monospace details |

Extraction evidence: the overlay structure occurs in ServerConfigModal,
LaunchFieldsModal and MemoryScannerModal; identical modal inputs occur at five JSX
sites, and the config input is a sixth site with a distinct existing variant.
These fields span three domains. Dialog buttons reuse Button's solid/outline variants.
Rows, tabs and progress bars differ in structure or have fewer than three identical
occurrences, so no additional primitive is introduced for them.

`scripts/design-utility-order.mjs` preserves Tailwind v3's original utility cascade
using the migration catalog and Tailwind's ordering API. This matters when a feature
adds a color to an existing Button, or combines text sizes or shadows. Renaming
classes alone changes alphabetical precedence even with identical declarations.
Keep this adapter through the rework; any later Tailwind upgrade must rerun CSS
and rendered-state comparisons. Shadow-color utilities are disabled because
shadows already own their colors and `shadow-panel` otherwise matches both plugins.

## Changing a theme

Change variables in `:root` (or override them under `:root[data-theme="..."]`)
and the shared primitive variants. Colors are RGB channels, not hex strings, so
`bg-panel/50`, hover/focus variants and other opacity modifiers keep working.
No rework values, new fonts or new theme behavior are applied by this refactor.

## Guard and verification

`npm run check:design` scans frontend source, scripts and build config. The migration
catalog is the only exemption: it must remember original utility names. The guard
rejects palette references, raw white/black overlays, pixel text sizes, legacy radii
and raw shadows, including state variants. It runs in the existing quality CI.
To check a fixture independently, use `npm run check:design -- /tmp/fixture.tsx`.

```sh
node scripts/migrate-design.mjs --check
npx tailwindcss --postcss -i src/index.css -o /tmp/ro-design-current.css
node scripts/design-css.mjs compare /tmp/ro-design-baseline.css /tmp/ro-design-current.css /tmp/ro-design-class-map.json
```

The original CSS remains outside the repo; see [the baseline](design-baseline.md)
for capture commands. For a fresh migration, `node scripts/migrate-design.mjs --map
/tmp/ro-design-class-map.json` emits the selector renaming evidence while retaining
variants and opacity. A second run makes zero edits.
