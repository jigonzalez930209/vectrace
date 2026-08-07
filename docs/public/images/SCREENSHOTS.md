# Screenshots for the docs site

Replace these **exact filenames** under `docs/public/images/` (keep the names so Markdown/README keep working). Use real PNG screenshots from Vectrace on your dual-monitor GNOME session.

| File to replace | What to capture | Suggested framing |
|-----------------|-----------------|-------------------|
| `hero-banner.png` | Full product hero: desktop with Vectrace overlay active + floating toolbar visible | Wide 16:9 or ultrawide crop of both monitors (or primary with toolbar). Show a few colorful strokes so it reads as a screen marker. |
| `toolbar-preview.png` | Close-up of the **floating glassmorphic toolbar** | Crop tightly around the toolbar (pen/shapes/colors/save). Soft desktop behind is fine. |
| `spotlight-demo.png` | **Spotlight mode** active | Dimmed desktop with a bright focus circle; optional laser trail. |
| `crop-selection.png` | **Crop / region selection** | Cyan/selection rectangle on screen with handles; exterior dimmed if your UI does that. |

## Optional (keep unless rebranding)

| File | Notes |
|------|--------|
| `logo.svg` | Brand mark (light) — cyan/clear glass |
| `logo-dark.svg` | Brand mark (dark) — same mark on dark glass |
| `vectrace-bg.svg` | Animated docs backdrop (theme asset, not a screenshot) |
| `vectrace-bg-dark.svg` | Dark-mode animated backdrop |

## Tips

1. Run Vectrace, draw a bit, enable the effect, then **Save full** or OS screenshot of the overlay.
2. Export as **PNG** and overwrite the file with the **same name** (even though some stubs were SVG misnamed `.png`).
3. Prefer ≥1600px wide for `hero-banner.png`; others can be ~1000–1400px.
4. After replacing, check locally: `pnpm docs:dev` → Home, Introduction, Installation, Special Effects, Snapshots.
5. README.md also embeds these four paths under `docs/public/images/`.
