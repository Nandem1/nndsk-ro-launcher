import { colorNames } from './scripts/design-token-map.mjs'

/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
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
      },
      fontSize: {
        micro: 'var(--text-micro)',
        caption: 'var(--text-caption)',
        detail: 'var(--text-detail)',
        label: 'var(--text-label)',
        ...Object.fromEntries(
          ['xs', 'sm', 'base', 'lg', 'xl'].map((size) => [
            size,
            [`var(--text-${size})`, { lineHeight: `var(--leading-${size})` }],
          ]),
        ),
      },
      backgroundImage: {
        'panel-gradient':
          'linear-gradient(var(--gradient-panel-direction), var(--tw-gradient-stops))',
        'progress-gradient':
          'linear-gradient(var(--gradient-progress-direction), var(--tw-gradient-stops))',
      },
      backdropBlur: { panel: 'var(--blur-panel)' },
      boxShadow: {
        panel: 'var(--shadow-panel)',
        modal: 'var(--shadow-modal)',
        control: 'var(--shadow-control)',
        'glow-warn': 'var(--shadow-glow-warn)',
        'glow-ok': 'var(--shadow-glow-ok)',
        'glow-bad': 'var(--shadow-glow-bad)',
        'dot-ok': 'var(--shadow-dot-ok)',
        'dot-warn': 'var(--shadow-dot-warn)',
        'dot-bad': 'var(--shadow-dot-bad)',
        'check-warn': 'var(--shadow-check-warn)',
        // Transitional aliases: removed by the mechanical migration.
        glass: 'var(--shadow-panel)',
        'glow-amber': 'var(--shadow-glow-warn)',
        'glow-emerald': 'var(--shadow-glow-ok)',
        'glow-red': 'var(--shadow-glow-bad)',
      },
      transitionTimingFunction: {
        'out-quart': 'cubic-bezier(0.25, 1, 0.5, 1)',
        spring: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
      },
      transitionDuration: {
        400: '400ms',
      },
      keyframes: {
        'fade-rise': {
          from: { opacity: '0', transform: 'translateY(8px)' },
          to: { opacity: '1', transform: 'none' },
        },
        'scale-in': {
          from: { opacity: '0', transform: 'scale(0.96)' },
          to: { opacity: '1', transform: 'none' },
        },
        'pulse-dot': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.4' },
        },
      },
      animation: {
        'fade-rise': 'fade-rise 0.4s cubic-bezier(0.25, 1, 0.5, 1) both',
        'scale-in': 'scale-in 0.18s cubic-bezier(0.25, 1, 0.5, 1) both',
        'pulse-dot': 'pulse-dot 2s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
