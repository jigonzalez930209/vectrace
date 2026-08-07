import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Vectrace',
  description: 'Agnostic Vector Screen Marker & Annotation Overlay for X11 & Wayland',
  base: '/vectrace/',
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/vectrace/images/logo.svg' }]
  ],
  themeConfig: {
    logo: '/images/logo.svg',
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Installation Guide', link: '/guide/installation' },
      { text: 'Features & Tools', link: '/guide/features' },
      { text: 'Architecture & CI', link: '/guide/architecture' }
    ],
    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Introduction', link: '/guide/introduction' },
          { text: 'Installation & Dependencies', link: '/guide/installation' },
          { text: 'Quickstart & Controls', link: '/guide/quickstart' }
        ]
      },
      {
        text: 'Core Functionality',
        items: [
          { text: 'Drawing & Shape Tools', link: '/guide/features' },
          { text: 'Spotlight & Neon Laser', link: '/guide/special-effects' },
          { text: 'Crop & Snapshot Engine', link: '/guide/snapshots' },
          { text: 'Click-Through & System Tray', link: '/guide/overlay-modes' }
        ]
      },
      {
        text: 'Deployment & Licensing',
        items: [
          { text: 'Architecture & Rendering', link: '/guide/architecture' },
          { text: 'CI/CD & GitHub Pages', link: '/guide/deploy' },
          { text: 'License & Legal Info', link: '/guide/license' }
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/jigonzalez930209/vectrace' }
    ],
    footer: {
      message: 'Released under the GNU General Public License v3.0 (GPL-3.0).',
      copyright: 'Copyright © 2026 Vectrace Developers'
    }
  }
})
