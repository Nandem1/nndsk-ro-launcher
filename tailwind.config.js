import { colorNames } from './scripts/design-token-map.mjs'

/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  // Shadows own their colors; avoid shadow-panel also becoming a color utility.
  corePlugins: { boxShadowColor: false },
  theme: {
    extend: {
      colors: Object.fromEntries(
        colorNames.map((name) => [
          name,
          `rgb(var(--c-${name}) / <alpha-value>)`,
        ]),
      ),
      borderRadius: {
        inline: 'var(--radius-inline)',
        'control-compact': 'var(--radius-control-compact)',
        control: 'var(--radius-control)',
        panel: 'var(--radius-panel)',
        modal: 'var(--radius-modal)',
        pill: 'var(--radius-pill)',
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
        wordmark: ['var(--font-wordmark)'],
      },
      fontSize: {
        micro: 'var(--text-micro)',
        caption: 'var(--text-caption)',
        detail: 'var(--text-detail)',
        label: 'var(--text-label)',
        'panel-title': ['var(--text-panel-title)', { lineHeight: '1.25rem' }],
        ...Object.fromEntries(
          ['xs', 'sm', 'base', 'lg', 'xl'].map((size) => [
            size,
            [`var(--text-${size})`, { lineHeight: `var(--leading-${size})` }],
          ]),
        ),
      },
      fontWeight: { 'panel-title': 'var(--weight-panel-title)' },
      letterSpacing: { 'panel-title': 'var(--tracking-panel-title)' },
      keyframes: {
        'modal-fade': {
          from: { opacity: '0' },
          to: { opacity: '1' },
        },
        'pulse-dot': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.4' },
        },
      },
      animation: {
        'modal-fade': 'modal-fade 0.12s ease-out both',
        'pulse-dot': 'pulse-dot 2s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
