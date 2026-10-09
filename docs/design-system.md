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

## Changing a theme

Change variables in `:root` (or override the same variables under a theme selector)
and the shared primitive variants. Colors are RGB channels, not hex strings, so
`bg-panel/50`, hover/focus variants and other opacity modifiers keep working.
No rework values, new fonts or new theme behavior are applied by this refactor.
