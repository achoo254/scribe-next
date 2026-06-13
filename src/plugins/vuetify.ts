// Vuetify + theme iNET One Series (dark-first). Màu lấy từ token ViUI vendor (products-one-*).
import 'vuetify/styles'
import { createVuetify, type ThemeDefinition } from 'vuetify'
import { aliases, tabler } from './tabler-iconset'
import { oneSeries, stateDark } from '../tokens/one-series'

// One Series dark — màu lấy từ token vendor (src/tokens/one-series.ts), KHÔNG hardcode hex tại đây.
const inetOneDark: ThemeDefinition = {
  dark: true,
  colors: {
    background: oneSeries.bg,
    surface: oneSeries.bgCard,
    'surface-bright': oneSeries.bgBorder,
    'surface-variant': oneSeries.bgBorder,
    primary: oneSeries.green,
    secondary: oneSeries.orange,
    accent: oneSeries.orange,
    error: stateDark.error,
    info: stateDark.info,
    success: stateDark.success,
    warning: stateDark.warning,
    'on-background': stateDark.text,
    'on-surface': stateDark.text,
    'on-primary': oneSeries.bg,
    'on-secondary': oneSeries.bg,
  },
}

export default createVuetify({
  theme: {
    defaultTheme: 'inetOneDark',
    themes: { inetOneDark },
  },
  icons: {
    defaultSet: 'tabler',
    aliases,
    sets: { tabler },
  },
  defaults: {
    global: { style: "font-family: 'Be Vietnam Pro', sans-serif" },
  },
})
