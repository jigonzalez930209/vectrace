import DefaultTheme from 'vitepress/theme'
import './custom.css'
import bgLight from './vectrace-bg.svg?url'
import bgDark from './vectrace-bg-dark.svg?url'

function applyBgVars() {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.style.setProperty('--vectrace-bg-light', `url("${bgLight}")`)
  root.style.setProperty('--vectrace-bg-dark', `url("${bgDark}")`)
}

export default {
  extends: DefaultTheme,
  enhanceApp() {
    applyBgVars()
  },
}
