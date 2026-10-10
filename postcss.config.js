import tailwindcss from 'tailwindcss'
import autoprefixer from 'autoprefixer'
import designUtilityOrder from './scripts/design-utility-order.mjs'

export default {
  plugins: [tailwindcss(), designUtilityOrder(), autoprefixer()],
}
